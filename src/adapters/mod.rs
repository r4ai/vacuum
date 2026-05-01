pub mod cargo;
pub mod gitignore;
pub mod go;
pub mod gradle;
pub mod maven;
pub mod node;
pub mod ocaml;
pub mod python;

pub use cargo::CargoAdapter;
pub use gitignore::GitignoreAdapter;
pub use go::GoAdapter;
pub use gradle::GradleAdapter;
pub use maven::MavenAdapter;
pub use node::NodeAdapter;
pub use ocaml::OcamlAdapter;
pub use python::PythonAdapter;
