//! The drawer's own picture (adr.rg.025): what the drawer shows, as an image on the
//! clipboard and a PNG under Pictures\ReviewGlass, so a diff can be pasted into a
//! conversation and talked about.
//!
//! The page is rendered by WebView2 itself (`CapturePreview`): no screen capture is
//! involved, so the picture is the drawer alone, whatever lies over or under it. The
//! page names the part to keep (the drawer, without the strip above it); the rest is
//! cut away here. Nothing is sent anywhere: the clipboard and one file.

use std::io::Cursor;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use webview2_com::CapturePreviewCompletedHandler;
use webview2_com::Microsoft::Web::WebView2::Win32::COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_PNG;
use windows::core::w;
use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL, HWND};
use windows::Win32::System::Com::{IStream, STREAM_SEEK_SET};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, OpenClipboard, RegisterClipboardFormatW, SetClipboardData,
};
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
use windows::Win32::System::SystemInformation::GetLocalTime;
use windows::Win32::UI::Shell::SHCreateMemStream;

/// The page is rendered on the main thread, which may be busy for a moment; a picture
/// that has not come in this long is not coming.
const CAPTURE_TIMEOUT: Duration = Duration::from_secs(5);

/// The standard device-independent bitmap format, for every application that does not
/// read the "PNG" format.
const CF_DIB: u32 = 8;

/// The part of the page to keep, in CSS pixels, with the page's own width so the
/// rendering's scale can be found.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct Crop {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub page_width: f64,
}

/// What became of one picture.
#[derive(Debug, Clone, Serialize)]
pub struct Picture {
    /// Where the PNG was saved; absent when it could not be.
    pub path: Option<String>,
    /// Whether the image is on the clipboard.
    pub copied: bool,
    /// What went wrong, in the user's terms, when something did.
    pub problem: Option<String>,
}

/// Take the drawer's picture: copy it to the clipboard and save it. An error only when
/// no picture could be taken at all; a picture that was taken but could not be copied
/// or saved says so in `problem`.
#[tauri::command]
pub async fn dock_picture(app: AppHandle, crop: Option<Crop>) -> Result<Picture, String> {
    let window = app
        .get_webview_window(crate::dock::DOCK_LABEL)
        .ok_or("the dock window is not there")?;
    let png = render(&window).await?;
    let (png, pixels) = cut(&png, crop)?;
    let dib = dib(&pixels);

    let mut problems = Vec::new();
    let copied = match window.hwnd() {
        Ok(hwnd) => match to_clipboard(HWND(hwnd.0), &png, &dib) {
            Ok(()) => true,
            Err(e) => {
                problems.push(format!("not copied: {e}"));
                false
            }
        },
        Err(e) => {
            problems.push(format!("not copied: {e}"));
            false
        }
    };
    let path = match save(&png) {
        Ok(p) => Some(p.display().to_string()),
        Err(e) => {
            problems.push(format!("not saved: {e}"));
            None
        }
    };
    Ok(Picture {
        path,
        copied,
        problem: (!problems.is_empty()).then(|| problems.join("; ")),
    })
}

/// Show a saved picture in its folder (Explorer, the file selected). Only a picture in
/// ReviewGlass's own folder: the page cannot open anything else through this.
#[tauri::command]
pub fn picture_show(path: String) -> Result<(), String> {
    let dir = pictures_dir().ok_or("no Pictures folder")?;
    let p = PathBuf::from(&path);
    if p.parent() != Some(dir.as_path()) || !p.is_file() {
        return Err("not a picture ReviewGlass saved".into());
    }
    std::process::Command::new("explorer.exe")
        .arg(format!("/select,{}", p.display()))
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Ask WebView2 to render the window's page as a PNG. The request is made on the main
/// thread (`with_webview`) and answered there later; the answer comes back through a
/// channel this task awaits.
async fn render(window: &tauri::WebviewWindow) -> Result<Vec<u8>, String> {
    let (tx, rx) = tokio::sync::oneshot::channel::<Result<Vec<u8>, String>>();
    let tx = Arc::new(Mutex::new(Some(tx)));
    let reply = move |r: Result<Vec<u8>, String>| {
        if let Some(t) = tx.lock().take() {
            let _ = t.send(r);
        }
    };
    window
        .with_webview(move |pw| {
            let answer = reply.clone();
            // SAFETY: COM calls on the webview's own (main) thread, with interfaces
            // WebView2 handed out; the stream lives in the handler until it is read.
            let started = unsafe {
                (|| -> windows::core::Result<()> {
                    let core = pw.controller().CoreWebView2()?;
                    let stream: IStream =
                        SHCreateMemStream(None).ok_or_else(windows::core::Error::empty)?;
                    let kept = stream.clone();
                    let handler = CapturePreviewCompletedHandler::create(Box::new(
                        move |result: windows::core::Result<()>| {
                            answer(
                                result
                                    .map_err(|e| e.message())
                                    .and_then(|()| read_stream(&kept)),
                            );
                            Ok(())
                        },
                    ));
                    core.CapturePreview(
                        COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_PNG,
                        &stream,
                        &handler,
                    )
                })()
            };
            if let Err(e) = started {
                reply(Err(e.message()));
            }
        })
        .map_err(|e| e.to_string())?;
    match tokio::time::timeout(CAPTURE_TIMEOUT, rx).await {
        Ok(Ok(r)) => r,
        Ok(Err(_)) => Err("the page's picture never came".into()),
        Err(_) => Err("the page did not render its picture in time".into()),
    }
}

/// Everything a stream holds, from its start.
fn read_stream(stream: &IStream) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let mut buf = vec![0u8; 64 * 1024];
    // SAFETY: `buf` outlives the calls and its length is what Read is told.
    unsafe {
        stream
            .Seek(0, STREAM_SEEK_SET, None)
            .map_err(|e| e.message())?;
        loop {
            let mut n = 0u32;
            stream
                .Read(buf.as_mut_ptr().cast(), buf.len() as u32, Some(&mut n))
                .ok()
                .map_err(|e| e.message())?;
            if n == 0 {
                break;
            }
            out.extend_from_slice(&buf[..n as usize]);
        }
    }
    Ok(out)
}

/// An image as 8-bit RGBA rows, top row first.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Pixels {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

fn decode(png_bytes: &[u8]) -> Result<Pixels, String> {
    let mut decoder = png::Decoder::new(Cursor::new(png_bytes));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let size = reader
        .output_buffer_size()
        .ok_or("the picture is too large")?;
    let mut buf = vec![0u8; size];
    let info = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
    let (w, h) = (info.width as usize, info.height as usize);
    let channels = match info.color_type {
        png::ColorType::Rgba => 4,
        png::ColorType::Rgb => 3,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Grayscale => 1,
        png::ColorType::Indexed => return Err("an indexed picture was not expanded".into()),
    };
    let mut rgba = Vec::with_capacity(w * h * 4);
    for y in 0..h {
        let row = &buf[y * info.line_size..y * info.line_size + w * channels];
        for px in row.chunks_exact(channels) {
            let (r, g, b, a) = match channels {
                4 => (px[0], px[1], px[2], px[3]),
                3 => (px[0], px[1], px[2], 255),
                2 => (px[0], px[0], px[0], px[1]),
                _ => (px[0], px[0], px[0], 255),
            };
            rgba.extend_from_slice(&[r, g, b, a]);
        }
    }
    Ok(Pixels {
        width: w as u32,
        height: h as u32,
        rgba,
    })
}

fn encode(p: &Pixels) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, p.width, p.height);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut writer = enc.write_header().map_err(|e| e.to_string())?;
    writer
        .write_image_data(&p.rgba)
        .map_err(|e| e.to_string())?;
    writer.finish().map_err(|e| e.to_string())?;
    Ok(out)
}

/// Keep the part the page named, scaled from CSS pixels to the rendering's own, and
/// return it as a PNG and as pixels. No crop, or one that misses the picture, keeps
/// the whole page.
fn cut(png_bytes: &[u8], crop: Option<Crop>) -> Result<(Vec<u8>, Pixels), String> {
    let all = decode(png_bytes)?;
    let Some(c) = crop.filter(|c| c.page_width > 0.0 && c.width > 0.0 && c.height > 0.0) else {
        return Ok((png_bytes.to_vec(), all));
    };
    let scale = all.width as f64 / c.page_width;
    let x0 = ((c.x * scale).round().max(0.0) as u32).min(all.width);
    let y0 = ((c.y * scale).round().max(0.0) as u32).min(all.height);
    let x1 = (((c.x + c.width) * scale).round().max(0.0) as u32).min(all.width);
    let y1 = (((c.y + c.height) * scale).round().max(0.0) as u32).min(all.height);
    if x1 <= x0 || y1 <= y0 {
        return Ok((png_bytes.to_vec(), all));
    }
    let stride = all.width as usize * 4;
    let mut rgba = Vec::with_capacity((x1 - x0) as usize * (y1 - y0) as usize * 4);
    for y in y0..y1 {
        let start = y as usize * stride + x0 as usize * 4;
        rgba.extend_from_slice(&all.rgba[start..start + (x1 - x0) as usize * 4]);
    }
    let part = Pixels {
        width: x1 - x0,
        height: y1 - y0,
        rgba,
    };
    Ok((encode(&part)?, part))
}

/// A packed device-independent bitmap: a 40-byte BITMAPINFOHEADER and 32-bit BGRA
/// rows, bottom row first, as CF_DIB wants them.
fn dib(p: &Pixels) -> Vec<u8> {
    let (w, h) = (p.width as usize, p.height as usize);
    let mut out = Vec::with_capacity(40 + w * h * 4);
    out.extend_from_slice(&40u32.to_le_bytes()); // biSize
    out.extend_from_slice(&(p.width as i32).to_le_bytes()); // biWidth
    out.extend_from_slice(&(p.height as i32).to_le_bytes()); // biHeight: bottom-up
    out.extend_from_slice(&1u16.to_le_bytes()); // biPlanes
    out.extend_from_slice(&32u16.to_le_bytes()); // biBitCount
    out.extend_from_slice(&0u32.to_le_bytes()); // biCompression: BI_RGB
    out.extend_from_slice(&((w * h * 4) as u32).to_le_bytes()); // biSizeImage
    out.extend_from_slice(&[0u8; 16]); // resolution and palette: none
    for y in (0..h).rev() {
        for px in p.rgba[y * w * 4..(y + 1) * w * 4].chunks_exact(4) {
            out.extend_from_slice(&[px[2], px[1], px[0], px[3]]);
        }
    }
    out
}

/// Put the picture on the clipboard in two forms: "PNG", which browsers and chat
/// applications read first, and CF_DIB for everything else.
fn to_clipboard(owner: HWND, png_bytes: &[u8], dib_bytes: &[u8]) -> Result<(), String> {
    // SAFETY: the clipboard is opened, filled and closed on this thread; a handle given
    // to SetClipboardData belongs to the system afterwards, and one it refused is
    // freed here.
    unsafe {
        let mut opened = OpenClipboard(Some(owner));
        for _ in 0..5 {
            if opened.is_ok() {
                break;
            }
            // Another application may hold the clipboard for a moment.
            std::thread::sleep(Duration::from_millis(40));
            opened = OpenClipboard(Some(owner));
        }
        opened.map_err(|e| format!("the clipboard is busy ({})", e.message()))?;
        let result = (|| -> Result<(), String> {
            EmptyClipboard().map_err(|e| e.message())?;
            let png_format = RegisterClipboardFormatW(w!("PNG"));
            if png_format != 0 {
                set_data(png_format, png_bytes)?;
            }
            set_data(CF_DIB, dib_bytes)
        })();
        let _ = CloseClipboard();
        result
    }
}

/// # Safety
/// The clipboard must be open on this thread.
unsafe fn set_data(format: u32, bytes: &[u8]) -> Result<(), String> {
    let h: HGLOBAL = GlobalAlloc(GMEM_MOVEABLE, bytes.len()).map_err(|e| e.message())?;
    let p = GlobalLock(h);
    if p.is_null() {
        let _ = GlobalFree(Some(h));
        return Err("no memory for the clipboard".into());
    }
    std::ptr::copy_nonoverlapping(bytes.as_ptr(), p.cast::<u8>(), bytes.len());
    let _ = GlobalUnlock(h);
    if let Err(e) = SetClipboardData(format, Some(HANDLE(h.0))) {
        let _ = GlobalFree(Some(h));
        return Err(e.message());
    }
    Ok(())
}

/// Save the PNG as Pictures\ReviewGlass\ReviewGlass-<date>-<time>.png.
fn save(png_bytes: &[u8]) -> Result<PathBuf, String> {
    let dir = pictures_dir().ok_or("no Pictures folder")?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    // SAFETY: GetLocalTime only fills the struct it returns.
    let t = unsafe { GetLocalTime() };
    let stamp = format!(
        "{:04}-{:02}-{:02}-{:02}{:02}{:02}",
        t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond
    );
    let mut path = dir.join(format!("ReviewGlass-{stamp}.png"));
    let mut n = 2;
    while path.exists() {
        path = dir.join(format!("ReviewGlass-{stamp}-{n}.png"));
        n += 1;
    }
    std::fs::write(&path, png_bytes).map_err(|e| e.to_string())?;
    Ok(path)
}

/// The user's Pictures folder (wherever Windows has it, OneDrive included), and
/// ReviewGlass's folder in it.
fn pictures_dir() -> Option<PathBuf> {
    dirs::picture_dir()
        .or_else(|| dirs::home_dir().map(|h| h.join("Pictures")))
        .map(|p| p.join("ReviewGlass"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 4×3 picture whose every pixel says where it is.
    fn sample() -> Pixels {
        let (w, h) = (4u32, 3u32);
        let mut rgba = Vec::new();
        for y in 0..h {
            for x in 0..w {
                rgba.extend_from_slice(&[x as u8 * 10, y as u8 * 10, 200, 255]);
            }
        }
        Pixels {
            width: w,
            height: h,
            rgba,
        }
    }

    #[test]
    fn a_picture_survives_the_round_trip_through_png() {
        let p = sample();
        assert_eq!(decode(&encode(&p).unwrap()).unwrap(), p);
    }

    #[test]
    fn the_crop_is_scaled_from_css_pixels_to_the_rendering() {
        let png_bytes = encode(&sample()).unwrap();
        // A page 2 CSS px wide rendered 4 px wide: scale 2. Keep x 1..2, y 0.5..1.5 CSS.
        let crop = Crop {
            x: 1.0,
            y: 0.5,
            width: 1.0,
            height: 1.0,
            page_width: 2.0,
        };
        let (out, part) = cut(&png_bytes, Some(crop)).unwrap();
        assert_eq!((part.width, part.height), (2, 2));
        // The top-left kept pixel is (2, 1) of the original.
        assert_eq!(&part.rgba[..4], &[20, 10, 200, 255]);
        assert_eq!(decode(&out).unwrap(), part);
        // No crop, or one outside the picture, keeps it whole.
        assert_eq!(cut(&png_bytes, None).unwrap().1, sample());
        let away = Crop { x: 50.0, ..crop };
        assert_eq!(cut(&png_bytes, Some(away)).unwrap().1, sample());
    }

    #[test]
    fn the_bitmap_is_bottom_up_bgra_behind_its_header() {
        let p = sample();
        let d = dib(&p);
        assert_eq!(d.len(), 40 + 4 * 3 * 4);
        assert_eq!(u32::from_le_bytes(d[0..4].try_into().unwrap()), 40);
        assert_eq!(i32::from_le_bytes(d[4..8].try_into().unwrap()), 4);
        assert_eq!(i32::from_le_bytes(d[8..12].try_into().unwrap()), 3);
        assert_eq!(u16::from_le_bytes(d[14..16].try_into().unwrap()), 32);
        // The first pixel stored is the bottom row's first: (0, 2), as BGRA.
        assert_eq!(&d[40..44], &[200, 20, 0, 255]);
    }

    #[test]
    fn pictures_go_to_a_folder_of_their_own() {
        let d = pictures_dir().unwrap();
        assert!(d.ends_with("ReviewGlass"));
    }
}
