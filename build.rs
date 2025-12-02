fn main() {
  // Apenas compila no Windows
  if cfg!(target_os = "windows") {
    let mut res = winres::WindowsResource::new();
    res.set_icon("assets/icon.ico"); // caminho do seu ícone .ico
    res.compile().unwrap();
  }
}
