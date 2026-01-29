use std::fs::File;
use std::io::BufWriter;

fn main() {
    // Create ICO from PNG with proper transparency
    if cfg!(target_os = "windows") {
        let icon_path = "assets/app.ico";
        
        // Load PNG and create ICO (use appicon.png for exe icon)
        let img = image::open("ui/assets/appicon.png").expect("Failed to load appicon.png");
        
        // Create multiple sizes for the ICO
        let sizes = [16, 32, 48, 256];
        let mut icon_dir = ico::IconDir::new(ico::ResourceType::Icon);
        
        for size in sizes {
            let resized = img.resize_exact(size, size, image::imageops::FilterType::Lanczos3);
            let rgba = resized.to_rgba8();
            let icon_image = ico::IconImage::from_rgba_data(size, size, rgba.into_raw());
            icon_dir.add_entry(ico::IconDirEntry::encode(&icon_image).unwrap());
        }
        
        // Write ICO file
        std::fs::create_dir_all("assets").ok();
        let file = File::create(icon_path).expect("Failed to create ICO file");
        icon_dir.write(BufWriter::new(file)).expect("Failed to write ICO");
        
        // Set up Windows resources
        let mut res = winres::WindowsResource::new();
        res.set_icon(icon_path);
        // 1:1 with C# app.manifest - Require Administrator privileges
        res.set_manifest(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <assemblyIdentity version="1.0.0.0" name="XillyGameMode.app"/>
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v2">
    <security>
      <requestedPrivileges xmlns="urn:schemas-microsoft-com:asm.v3">
        <requestedExecutionLevel level="requireAdministrator" uiAccess="false" />
      </requestedPrivileges>
    </security>
  </trustInfo>
  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1">
    <application>
      <!-- Windows 10/11 -->
      <supportedOS Id="{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}"/>
    </application>
  </compatibility>
</assembly>
"#);
        res.compile().unwrap();
    }
    
    slint_build::compile("ui/app-window.slint").unwrap();
}
