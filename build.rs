fn main() {
    tonic_build::compile_protos("src/proto/storage.proto").expect("Failed to compile proto");
}
