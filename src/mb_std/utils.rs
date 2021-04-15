use std::fs;
use xmas_elf::ElfFile;
pub(super) fn process_elf<F: FnMut(&ElfFile, &str) -> Result<(), String>>(
    file: &str,
    mut f: F,
) -> Result<(), String> {
    let file_expand = shellexpand::full(file)
        .map_err(|e| e.to_string())?
        .to_string();
    let content = fs::read(&file_expand)
        .map_err(|e| format!("process elf {} fail! {:?}", &file_expand, e.to_string()))?
        .into_boxed_slice();
    let elf = ElfFile::new(&content)
        .map_err(|e| format!("process elf {} fail! {:?}", &file_expand, e.to_string()))?;
    f(&elf, &file_expand)
        .map_err(|e| format!("process elf {} fail! {:?}", &file_expand, e.to_string()))
}
