fn main() {
    #[cfg(windows)]
    {
        println!("cargo:rerun-if-changed=assets/icons/ncmora.ico");
        println!("cargo:rerun-if-changed=assets/icons/ncmora.rc");
        let _ = embed_resource::compile("assets/icons/ncmora.rc", embed_resource::NONE);
    }
}
