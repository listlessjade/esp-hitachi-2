fn main() {
    embuild::espidf::sysenv::output();
    println!("cargo::rustc-check-cfg=cfg(esp_idf_log_colors)");
    println!("cargo::rustc-check-cfg=cfg(esp_idf_log_timestamp_source_rtos)");
    println!("cargo::rustc-check-cfg=cfg(esp_idf_log_timestamp_source_system)");
}
