use crate::db::{Asset, Database};
use crate::error::AppResult;
use image::imageops::FilterType;
use image::{GenericImageView, RgbaImage, Rgba};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaterialInfo {
    pub shader_name: Option<String>,
    pub textures: Vec<MaterialTexture>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaterialTexture {
    pub slot_name: String,
    pub texture_guid: Option<String>,
    pub texture_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub vertex_count: Option<u64>,
    pub triangle_count: Option<u64>,
    pub submesh_count: Option<u32>,
    pub has_normals: bool,
    pub has_uvs: bool,
    pub bounds: Option<[f32; 6]>, // min_x, min_y, min_z, max_x, max_y, max_z
}

pub struct PreviewGenerator {
    db: Arc<Database>,
    thumbnail_dir: PathBuf,
    thumbnail_size: u32,
}

impl PreviewGenerator {
    pub fn new(db: Arc<Database>, thumbnail_dir: PathBuf, thumbnail_size: u32) -> Self {
        Self {
            db,
            thumbnail_dir,
            thumbnail_size,
        }
    }

    pub fn generate_thumbnail(&self, asset: &Asset) -> AppResult<Option<String>> {
        match asset.asset_type.as_str() {
            "texture" => self.generate_texture_thumbnail(asset),
            "material" => self.generate_material_thumbnail(asset),
            _ => Ok(None),
        }
    }

    fn generate_texture_thumbnail(&self, asset: &Asset) -> AppResult<Option<String>> {
        let source_path = Path::new(&asset.absolute_path);

        // Check if we support this format
        let extension = source_path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();

        let is_psd = extension == "psd";
        if !is_psd {
            match extension.as_str() {
                "png" | "jpg" | "jpeg" | "tga" | "bmp" | "gif" => {}
                _ => return Ok(None),
            }
        }

        // Generate a unique filename
        let thumb_name = format!(
            "{:x}_{}.png",
            md5_hash(&asset.absolute_path),
            asset.modified_time
        );
        let thumb_path = self.thumbnail_dir.join(&thumb_name);

        // Check if thumbnail already exists
        if thumb_path.exists() {
            let thumb_path_str = thumb_path.to_string_lossy().to_string();
            self.db.update_asset_thumbnail(&asset.id, &thumb_path_str)?;
            return Ok(Some(thumb_path_str));
        }

        // Load and resize image
        let img = if is_psd {
            // Handle PSD files - wrap in catch_unwind since psd crate can panic on some files
            match std::fs::read(source_path) {
                Ok(data) => {
                    use std::panic::AssertUnwindSafe;
                    let data_ref = &data;
                    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
                        psd::Psd::from_bytes(data_ref).ok().and_then(|psd| {
                            let rgba = psd.rgba();
                            let width = psd.width();
                            let height = psd.height();
                            image::RgbaImage::from_raw(width, height, rgba)
                        })
                    }));

                    match result {
                        Ok(Some(img)) => image::DynamicImage::ImageRgba8(img),
                        Ok(None) => {
                            tracing::warn!("Failed to parse PSD {}", asset.absolute_path);
                            return Ok(None);
                        }
                        Err(_) => {
                            tracing::warn!("PSD parsing panicked for {}", asset.absolute_path);
                            return Ok(None);
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to read PSD {}: {}", asset.absolute_path, e);
                    return Ok(None);
                }
            }
        } else {
            match image::open(source_path) {
                Ok(img) => img,
                Err(e) => {
                    tracing::warn!("Failed to open image {}: {}", asset.absolute_path, e);
                    return Ok(None);
                }
            }
        };

        let (width, height) = img.dimensions();
        if width == 0 || height == 0 {
            return Ok(None);
        }

        // Calculate resize dimensions maintaining aspect ratio
        let (new_width, new_height) = if width > height {
            let h = (height as f32 * self.thumbnail_size as f32 / width as f32) as u32;
            (self.thumbnail_size, h.max(1))
        } else {
            let w = (width as f32 * self.thumbnail_size as f32 / height as f32) as u32;
            (w.max(1), self.thumbnail_size)
        };

        let resized = img.resize_exact(new_width, new_height, FilterType::Triangle);

        // Save as PNG (better quality for thumbnails)
        if let Err(e) = resized.save(&thumb_path) {
            tracing::warn!("Failed to save thumbnail {}: {}", thumb_path.display(), e);
            return Ok(None);
        }

        let thumb_path_str = thumb_path.to_string_lossy().to_string();
        self.db.update_asset_thumbnail(&asset.id, &thumb_path_str)?;

        Ok(Some(thumb_path_str))
    }

    fn generate_material_thumbnail(&self, asset: &Asset) -> AppResult<Option<String>> {
        // Parse material to get texture info
        let mat_info = match parse_material_file(Path::new(&asset.absolute_path)) {
            Some(info) => info,
            None => return Ok(None),
        };

        if mat_info.textures.is_empty() {
            return Ok(None);
        }

        // Generate thumbnail name
        let thumb_name = format!(
            "mat_{:x}_{}.png",
            md5_hash(&asset.absolute_path),
            asset.modified_time
        );
        let thumb_path = self.thumbnail_dir.join(&thumb_name);

        if thumb_path.exists() {
            let thumb_path_str = thumb_path.to_string_lossy().to_string();
            self.db.update_asset_thumbnail(&asset.id, &thumb_path_str)?;
            return Ok(Some(thumb_path_str));
        }

        // Try to find and load the main texture (albedo/diffuse)
        let main_texture = mat_info.textures.iter().find(|t| {
            let slot = t.slot_name.to_lowercase();
            slot.contains("albedo") || slot.contains("diffuse") || slot.contains("maintex") || slot.contains("base")
        }).or_else(|| mat_info.textures.first());

        if let Some(tex) = main_texture {
            if let Some(guid) = &tex.texture_guid {
                // Look up texture by GUID
                if let Ok(Some(tex_asset)) = self.db.get_asset_by_guid(&asset.project_id, guid) {
                    // Generate thumbnail from that texture
                    if let Ok(Some(thumb)) = self.generate_texture_thumbnail(&tex_asset) {
                        // Copy/link to material thumbnail
                        if let Err(e) = fs::copy(&thumb, &thumb_path) {
                            tracing::warn!("Failed to copy material thumbnail: {}", e);
                        } else {
                            let thumb_path_str = thumb_path.to_string_lossy().to_string();
                            self.db.update_asset_thumbnail(&asset.id, &thumb_path_str)?;
                            return Ok(Some(thumb_path_str));
                        }
                    }
                }
            }
        }

        // If we couldn't find a texture, create a colored placeholder based on material properties
        let placeholder = create_material_placeholder(&mat_info, self.thumbnail_size);
        if let Err(e) = placeholder.save(&thumb_path) {
            tracing::warn!("Failed to save material placeholder: {}", e);
            return Ok(None);
        }

        let thumb_path_str = thumb_path.to_string_lossy().to_string();
        self.db.update_asset_thumbnail(&asset.id, &thumb_path_str)?;
        Ok(Some(thumb_path_str))
    }

    pub fn generate_thumbnails_for_project(&self, project_id: &str, limit: i64) -> AppResult<usize> {
        let assets = self.db.get_assets_needing_thumbnails(project_id, limit)?;
        let mut generated = 0;

        for asset in assets {
            match self.generate_thumbnail(&asset) {
                Ok(Some(_)) => generated += 1,
                Ok(None) => {}
                Err(e) => {
                    tracing::warn!("Failed to generate thumbnail for {}: {}", asset.relative_path, e);
                }
            }
        }

        Ok(generated)
    }
}

/// Parse a Unity .mat file to extract texture references
pub fn parse_material_file(path: &Path) -> Option<MaterialInfo> {
    let content = fs::read_to_string(path).ok()?;

    let mut info = MaterialInfo {
        shader_name: None,
        textures: Vec::new(),
    };

    // Extract shader name
    let shader_re = Regex::new(r"m_Shader:\s*\{[^}]*\}").ok()?;
    if let Some(_) = shader_re.find(&content) {
        // Try to find shader name in the file
        let name_re = Regex::new(r#"m_Name:\s*([^\n\r]+)"#).ok()?;
        if let Some(cap) = name_re.captures(&content) {
            info.shader_name = Some(cap.get(1)?.as_str().trim().to_string());
        }
    }

    // Extract texture properties
    // Unity materials have sections like:
    // - _MainTex:
    //     m_Texture: {fileID: 2800000, guid: abc123..., type: 3}
    let tex_section_re = Regex::new(r"- (\w+):\s*\n\s*m_Texture:\s*\{[^}]*guid:\s*([a-f0-9]{32})").ok()?;

    for cap in tex_section_re.captures_iter(&content) {
        let slot_name = cap.get(1)?.as_str().to_string();
        let guid = cap.get(2)?.as_str().to_string();

        info.textures.push(MaterialTexture {
            slot_name,
            texture_guid: Some(guid),
            texture_path: None,
        });
    }

    // Also try alternate format without guid (embedded or null textures)
    let tex_name_re = Regex::new(r"- (\w+):\s*\n\s*m_Texture:").ok()?;
    for cap in tex_name_re.captures_iter(&content) {
        let slot_name = cap.get(1)?.as_str().to_string();
        // Only add if not already present
        if !info.textures.iter().any(|t| t.slot_name == slot_name) {
            info.textures.push(MaterialTexture {
                slot_name,
                texture_guid: None,
                texture_path: None,
            });
        }
    }

    Some(info)
}

/// Parse basic model info from supported formats
pub fn parse_model_info(path: &Path) -> Option<ModelInfo> {
    let extension = path.extension()?.to_str()?.to_lowercase();

    match extension.as_str() {
        "obj" => parse_obj_info(path),
        "gltf" | "glb" => parse_gltf_info(path),
        "fbx" => parse_fbx_info(path),
        "dae" => parse_dae_info(path),
        "blend" => Some(ModelInfo {
            vertex_count: None,
            triangle_count: None,
            submesh_count: None,
            has_normals: true,
            has_uvs: true,
            bounds: None,
        }),
        _ => None,
    }
}

fn parse_fbx_info(path: &Path) -> Option<ModelInfo> {
    // FBX is a complex binary/ASCII format
    // We'll do basic parsing to extract what we can
    let data = fs::read(path).ok()?;

    // Check if it's binary FBX (starts with "Kaydara FBX Binary")
    let is_binary = data.len() > 20 && &data[0..18] == b"Kaydara FBX Binary";

    if is_binary {
        // Binary FBX - try to find vertex/polygon counts in the data
        // This is a simplified heuristic
        let mut vertex_count = None;
        let mut polygon_count = None;

        // Look for "Vertices" and "PolygonVertexIndex" nodes
        // In binary FBX, strings are null-terminated after a length byte
        let data_str = String::from_utf8_lossy(&data);

        // Try to find vertex count hints
        if data_str.contains("Vertices") {
            // Very rough estimate based on file size
            let estimated_verts = (data.len() / 50) as u64; // rough heuristic
            vertex_count = Some(estimated_verts.min(1_000_000));
        }

        if data_str.contains("PolygonVertexIndex") {
            let estimated_polys = (data.len() / 150) as u64;
            polygon_count = Some(estimated_polys.min(500_000));
        }

        Some(ModelInfo {
            vertex_count,
            triangle_count: polygon_count,
            submesh_count: None,
            has_normals: data_str.contains("Normals"),
            has_uvs: data_str.contains("UV"),
            bounds: None,
        })
    } else {
        // ASCII FBX
        let content = String::from_utf8_lossy(&data);

        let mut vertex_count = 0u64;
        let mut has_normals = false;
        let mut has_uvs = false;

        // Count vertices in ASCII FBX
        for line in content.lines() {
            let line = line.trim();
            if line.starts_with("Vertices:") {
                // Try to parse vertex count from the line
                if let Some(rest) = line.strip_prefix("Vertices: *") {
                    if let Some(count_str) = rest.split_whitespace().next() {
                        if let Ok(count) = count_str.parse::<u64>() {
                            vertex_count = count / 3; // 3 floats per vertex
                        }
                    }
                }
            }
            if line.contains("Normals:") || line.contains("LayerElementNormal") {
                has_normals = true;
            }
            if line.contains("UV:") || line.contains("LayerElementUV") {
                has_uvs = true;
            }
        }

        Some(ModelInfo {
            vertex_count: if vertex_count > 0 { Some(vertex_count) } else { None },
            triangle_count: None,
            submesh_count: None,
            has_normals,
            has_uvs,
            bounds: None,
        })
    }
}

fn parse_dae_info(path: &Path) -> Option<ModelInfo> {
    // COLLADA (DAE) is XML-based
    let content = fs::read_to_string(path).ok()?;

    let mut vertex_count = 0u64;
    let mut triangle_count = 0u64;
    let has_normals = content.contains("<source") && content.contains("NORMAL");
    let has_uvs = content.contains("TEXCOORD");

    // Try to find float_array with positions
    // Format: <float_array id="...-positions-array" count="123">
    let pos_re = regex::Regex::new(r#"positions-array"?\s+count="(\d+)""#).ok()?;
    if let Some(cap) = pos_re.captures(&content) {
        if let Ok(count) = cap.get(1)?.as_str().parse::<u64>() {
            vertex_count = count / 3;
        }
    }

    // Try to find triangle count
    // Format: <triangles count="123">
    let tri_re = regex::Regex::new(r#"<triangles[^>]*count="(\d+)""#).ok()?;
    for cap in tri_re.captures_iter(&content) {
        if let Ok(count) = cap.get(1).map(|m| m.as_str()).unwrap_or("0").parse::<u64>() {
            triangle_count += count;
        }
    }

    Some(ModelInfo {
        vertex_count: if vertex_count > 0 { Some(vertex_count) } else { None },
        triangle_count: if triangle_count > 0 { Some(triangle_count) } else { None },
        submesh_count: None,
        has_normals,
        has_uvs,
        bounds: None,
    })
}

fn parse_obj_info(path: &Path) -> Option<ModelInfo> {
    let content = fs::read_to_string(path).ok()?;

    let mut vertex_count = 0u64;
    let mut face_count = 0u64;
    let mut has_normals = false;
    let mut has_uvs = false;

    for line in content.lines() {
        let line = line.trim();
        if line.starts_with("v ") {
            vertex_count += 1;
        } else if line.starts_with("f ") {
            // Count triangles - faces can be quads or ngons
            let parts: Vec<&str> = line.split_whitespace().skip(1).collect();
            if parts.len() >= 3 {
                face_count += (parts.len() - 2) as u64; // Triangulate
            }
        } else if line.starts_with("vn ") {
            has_normals = true;
        } else if line.starts_with("vt ") {
            has_uvs = true;
        }
    }

    Some(ModelInfo {
        vertex_count: Some(vertex_count),
        triangle_count: Some(face_count),
        submesh_count: Some(1),
        has_normals,
        has_uvs,
        bounds: None,
    })
}

fn parse_gltf_info(path: &Path) -> Option<ModelInfo> {
    // For glTF, we'd need to parse the JSON structure
    // This is a simplified version that just checks if the file exists
    let extension = path.extension()?.to_str()?;

    if extension == "glb" {
        // Binary glTF - would need proper parsing
        let metadata = fs::metadata(path).ok()?;
        if metadata.len() > 0 {
            return Some(ModelInfo {
                vertex_count: None,
                triangle_count: None,
                submesh_count: None,
                has_normals: true,
                has_uvs: true,
                bounds: None,
            });
        }
    } else {
        // JSON glTF
        let content = fs::read_to_string(path).ok()?;
        if content.contains("\"meshes\"") {
            return Some(ModelInfo {
                vertex_count: None,
                triangle_count: None,
                submesh_count: None,
                has_normals: true,
                has_uvs: true,
                bounds: None,
            });
        }
    }

    None
}

/// Create a placeholder image for materials without loadable textures
fn create_material_placeholder(info: &MaterialInfo, size: u32) -> RgbaImage {
    let mut img = RgbaImage::new(size, size);

    // Base color - gradient based on texture slots
    let has_normal = info.textures.iter().any(|t| t.slot_name.to_lowercase().contains("normal") || t.slot_name.to_lowercase().contains("bump"));
    let has_metallic = info.textures.iter().any(|t| t.slot_name.to_lowercase().contains("metallic") || t.slot_name.to_lowercase().contains("specular"));
    let has_emission = info.textures.iter().any(|t| t.slot_name.to_lowercase().contains("emission"));

    // Create a stylized material icon
    for y in 0..size {
        for x in 0..size {
            let fx = x as f32 / size as f32;
            let fy = y as f32 / size as f32;

            // Sphere-like shading
            let cx = fx - 0.5;
            let cy = fy - 0.5;
            let dist = (cx * cx + cy * cy).sqrt();

            if dist < 0.45 {
                let shade = 1.0 - (dist / 0.45);
                let highlight = if cx + cy < -0.2 { 0.3 } else { 0.0 };

                let r = if has_emission { 200 } else { 100 };
                let g = if has_metallic { 120 } else { 100 };
                let b = if has_normal { 140 } else { 120 };

                let r = ((r as f32 * shade + highlight * 255.0).min(255.0)) as u8;
                let g = ((g as f32 * shade + highlight * 255.0).min(255.0)) as u8;
                let b = ((b as f32 * shade + highlight * 255.0).min(255.0)) as u8;

                img.put_pixel(x, y, Rgba([r, g, b, 255]));
            } else {
                img.put_pixel(x, y, Rgba([30, 30, 35, 255]));
            }
        }
    }

    img
}

fn md5_hash(input: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    input.hash(&mut hasher);
    hasher.finish()
}
