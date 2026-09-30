fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let mut resource = tauri_winres::WindowsResource::new();
    resource
        .set_icon("../../assets/cs2-resedit.ico")
        .set_language(0x0409)
        .set("FileDescription", "CS2 ResEdit")
        .set("ProductName", "CS2 ResEdit")
        .set("CompanyName", "Softhe")
        .set("LegalCopyright", "Copyright \u{a9} Softhe")
        .set("OriginalFilename", "CS2-ResEdit.exe");

    let version = std::env::var("CARGO_PKG_VERSION").unwrap_or_default();
    let parts: Vec<u64> = version
        .split('.')
        .map(|part| part.parse().unwrap_or(0))
        .collect();
    let word = |index: usize| parts.get(index).copied().unwrap_or(0);
    let packed = word(0) << 48 | word(1) << 32 | word(2) << 16;
    resource.set_version_info(tauri_winres::VersionInfo::FILEVERSION, packed);
    resource.set_version_info(tauri_winres::VersionInfo::PRODUCTVERSION, packed);

    resource.compile().expect(
        "Windows resource compilation failed; the release requires its icon and version metadata",
    );
}
