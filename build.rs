fn main() {
    tonic_prost_build::compile_protos("src/proto/storage.proto").expect("Failed to compile proto");
}

