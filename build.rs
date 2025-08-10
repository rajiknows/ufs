fn main() {
    tonic_build::configure()
        .type_attribute(
            "FileInfo",
            "#[derive(serde::Serialize, serde::Deserialize)]",
        )
        .compile_protos(&["src/proto/storage.proto"], &["src/proto"])
        .expect("Failed to compile proto");
}
