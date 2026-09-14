use std::path::Path;

use image::ImageFormat;

use crate::error::Result;

/// 为曲谱图片生成缩略图（JPG，最长边不超过 480px）
pub fn generate_thumbnail(src: &Path, dest: &Path) -> Result<()> {
    let img = image::open(src)?;
    let thumb = img.thumbnail(480, 480);
    thumb.save_with_format(dest, ImageFormat::Jpeg)?;
    Ok(())
}

/// 将内存中的图片字节保存到目标位置（按扩展名格式重编码）
pub fn save_image_bytes(bytes: &[u8], dest: &Path, format: ImageFormat) -> Result<()> {
    let img = image::load_from_memory(bytes)?;
    let mut out = std::fs::File::create(dest)?;
    img.write_to(&mut out, format)?;
    Ok(())
}
