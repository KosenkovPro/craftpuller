//! Процедурная иконка приложения и установка ярлыка на рабочий стол.

use std::path::{Path, PathBuf};

/// Состояние иконки.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconKind {
    Idle,
    Working,
    Paused,
    Error,
}

impl IconKind {
    fn colors(self) -> ([u8; 3], [u8; 3]) {
        match self {
            IconKind::Idle => ([0x4C, 0x8D, 0xFF], [0x1D, 0x4E, 0xD8]),
            IconKind::Working => ([0x34, 0xD3, 0x99], [0x15, 0x80, 0x3D]),
            IconKind::Paused => ([0xFB, 0xBF, 0x24], [0xB4, 0x53, 0x09]),
            IconKind::Error => ([0xF8, 0x71, 0x71], [0xB9, 0x1C, 0x1C]),
        }
    }
}

/// Линейная интерполяция (f32::lerp ещё не стабилен).
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Нарисовать иконку в RGBA.
pub fn make_rgba(size: u32, kind: IconKind) -> Vec<u8> {
    let (c1, c2) = kind.colors();
    let ss = 3usize; // сглаживание
    let mut out = vec![0u8; (size * size * 4) as usize];

    for py in 0..size {
        for px in 0..size {
            let mut bg_hits = 0f32;
            let mut arrow_hits = 0f32;
            for sy in 0..ss {
                for sx in 0..ss {
                    let fx = (px as f32 + (sx as f32 + 0.5) / ss as f32) / size as f32;
                    let fy = (py as f32 + (sy as f32 + 0.5) / ss as f32) / size as f32;
                    if inside_rounded_rect(fx, fy, 0.22) {
                        bg_hits += 1.0;
                        if inside_arrow(fx, fy) {
                            arrow_hits += 1.0;
                        }
                    }
                }
            }
            let total = (ss * ss) as f32;
            let bg_cov = bg_hits / total;
            let arrow_cov = arrow_hits / total;
            if bg_cov <= 0.0 {
                continue;
            }
            // Вертикальный градиент фона.
            let t = py as f32 / size as f32;
            let mut r = lerp(c1[0] as f32, c2[0] as f32, t);
            let mut g = lerp(c1[1] as f32, c2[1] as f32, t);
            let mut b = lerp(c1[2] as f32, c2[2] as f32, t);
            // Белая стрелка.
            r = lerp(r, 255.0, arrow_cov);
            g = lerp(g, 255.0, arrow_cov);
            b = lerp(b, 255.0, arrow_cov);

            let i = ((py * size + px) * 4) as usize;
            out[i] = r as u8;
            out[i + 1] = g as u8;
            out[i + 2] = b as u8;
            out[i + 3] = (bg_cov * 255.0) as u8;
        }
    }
    out
}

fn inside_rounded_rect(x: f32, y: f32, r: f32) -> bool {
    let cx = if x < r {
        r
    } else if x > 1.0 - r {
        1.0 - r
    } else {
        x
    };
    let cy = if y < r {
        r
    } else if y > 1.0 - r {
        1.0 - r
    } else {
        y
    };
    let dx = x - cx;
    let dy = y - cy;
    dx * dx + dy * dy <= r * r
}

fn inside_arrow(x: f32, y: f32) -> bool {
    if x >= 0.435 && x <= 0.565 && y >= 0.16 && y <= 0.56 {
        return true;
    }
    if y >= 0.46 && y <= 0.80 {
        let t = (y - 0.46) / 0.34;
        let half = 0.21 * (1.0 - t);
        if x >= 0.5 - half && x <= 0.5 + half {
            return true;
        }
    }
    false
}

/// Иконка окна.
pub fn window_icon() -> egui::IconData {
    let size = 64u32;
    egui::IconData {
        rgba: make_rgba(size, IconKind::Idle),
        width: size,
        height: size,
    }
}

/// Записать .ico файл.
pub fn write_ico(path: &Path, size: u32, rgba: &[u8]) -> std::io::Result<()> {
    let px = size as usize;
    // BGRA, снизу вверх.
    let mut xor = Vec::with_capacity(px * px * 4);
    for y in (0..px).rev() {
        for x in 0..px {
            let i = (y * px + x) * 4;
            xor.push(rgba[i + 2]);
            xor.push(rgba[i + 1]);
            xor.push(rgba[i]);
            xor.push(rgba[i + 3]);
        }
    }
    // AND-маска: 1 бит на пиксель, строки кратны 4 байтам.
    let mask_row = ((px + 31) / 32) * 4;
    let and = vec![0u8; mask_row * px];

    let mut dib = Vec::new();
    dib.extend_from_slice(&40u32.to_le_bytes());
    dib.extend_from_slice(&(px as i32).to_le_bytes());
    dib.extend_from_slice(&((px * 2) as i32).to_le_bytes());
    dib.extend_from_slice(&1u16.to_le_bytes());
    dib.extend_from_slice(&32u16.to_le_bytes());
    dib.extend_from_slice(&0u32.to_le_bytes());
    dib.extend_from_slice(&((xor.len() + and.len()) as u32).to_le_bytes());
    dib.extend_from_slice(&0u32.to_le_bytes());
    dib.extend_from_slice(&0u32.to_le_bytes());
    dib.extend_from_slice(&0u32.to_le_bytes());
    dib.extend_from_slice(&0u32.to_le_bytes());
    let image = [dib, xor, and].concat();

    let mut ico = Vec::new();
    ico.extend_from_slice(&0u16.to_le_bytes());
    ico.extend_from_slice(&1u16.to_le_bytes());
    ico.extend_from_slice(&1u16.to_le_bytes());
    ico.extend_from_slice(&[(px.min(256)) as u8, (px.min(256)) as u8, 0, 0]);
    ico.extend_from_slice(&1u16.to_le_bytes());
    ico.extend_from_slice(&32u16.to_le_bytes());
    ico.extend_from_slice(&(image.len() as u32).to_le_bytes());
    ico.extend_from_slice(&22u32.to_le_bytes());
    ico.extend_from_slice(&image);
    std::fs::write(path, ico)
}

/// Записать PNG-иконку (для сборок под Linux и для документации).
pub fn write_png(path: &Path, size: u32, rgba: &[u8]) -> std::io::Result<()> {
    let img = image::RgbaImage::from_raw(size, size, rgba.to_vec())
        .ok_or_else(|| std::io::Error::other(crate::lang::t("err.icon_build")))?;
    img.save(path)
        .map_err(|e| std::io::Error::other(e.to_string()))
}

/// Куда устанавливаем программу.
pub fn install_dir() -> PathBuf {
    let base = dirs::data_local_dir()
        .or_else(dirs::data_dir)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("Programs").join("CraftPull")
}

/// Путь к установленному исполняемому файлу.
pub fn installed_exe() -> PathBuf {
    install_dir().join("craftpull.exe")
}

/// Создать ярлык на рабочем столе и (по возможности) установить программу.
pub fn install_shortcut() -> Result<PathBuf, String> {
    let dir = install_dir();
    std::fs::create_dir_all(&dir).map_err(|e| {
        crate::lang::tf(
            "err.shortcut_dir",
            &[("{dir}", &dir.display().to_string()), ("{e}", &e.to_string())],
        )
    })?;

    let current = std::env::current_exe().map_err(|e| format!("{e}"))?;
    let target = installed_exe();
    // Копируем текущую сборку в папку установки, чтобы обновление было стабильным.
    if let Err(e) = std::fs::copy(&current, &target) {
        if e.kind() != std::io::ErrorKind::AlreadyExists {
            return Err(crate::lang::tf("err.shortcut_copy", &[("{e}", &e.to_string())]));
        }
    }

    let ico_path = dir.join("craftpull.ico");
    let rgba = make_rgba(256, IconKind::Idle);
    write_ico(&ico_path, 256, &rgba)
        .map_err(|e| crate::lang::tf("err.shortcut_ico", &[("{e}", &e.to_string())]))?;

    let desktop = dirs::desktop_dir()
        .ok_or_else(|| crate::lang::t("err.shortcut_desktop").to_string())?;
    std::fs::create_dir_all(&desktop).ok();
    let lnk = desktop.join("CraftPull.lnk");

    let script = format!(
        "$ErrorActionPreference='Stop'\n\
         $dir=[Environment]::GetFolderPath('Desktop')\n\
         $lnk=Join-Path $dir 'CraftPull.lnk'\n\
         $s=(New-Object -ComObject WScript.Shell).CreateShortcut($lnk)\n\
         $s.TargetPath='{target}'\n\
         $s.WorkingDirectory='{dir}'\n\
         $s.IconLocation='{ico},0'\n\
         $s.Description='CraftPull - release and source manager'\n\
         $s.Save()\n\
         Write-Output $lnk\n",
        target = target.to_string_lossy().replace('\'', "''"),
        dir = dir.to_string_lossy().replace('\'', "''"),
        ico = ico_path.to_string_lossy().replace('\'', "''"),
    );

    let script_path = dir.join("make-shortcut.ps1");
    std::fs::write(&script_path, &script).map_err(|e| format!("{e}"))?;

    let output = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
            &script_path.to_string_lossy(),
        ])
        .output()
        .map_err(|e| crate::lang::tf("err.shortcut_ps", &[("{e}", &e.to_string())]))?;

    if !output.status.success() {
        return Err(crate::lang::tf(
            "err.shortcut_ps_fail",
            &[("{e}", String::from_utf8_lossy(&output.stderr).trim())],
        ));
    }
    let _ = std::fs::remove_file(&script_path);
    Ok(lnk)
}
