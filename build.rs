fn main() {
    #[cfg(windows)]
    {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("assets/icons/redsamurai.ico");
        resource
            .compile()
            .expect("failed to compile the Windows application icon");
    }
}
