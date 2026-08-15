use std::collections::HashMap;
use std::fs::write;
use std::path::PathBuf;

use pyo3::{Bound, Py, PyResult, PyTraverseError, PyVisit, pyclass, pymethods};

use crate::anim::Anim;
use crate::math::{Quat, Vec3};
use crate::model::Model;
use crate::render::{CameraAngle, RenderOutput};
use crate::tools::{export_glb, render};
use crate::utils::Dict;

#[pyclass]
#[derive(Default)]
pub struct Scene {
    pub anims: Vec<Py<Anim>>,
    pub nodes: Vec<Py<Node>>,
    pub meshes: Vec<Py<Mesh>>,
}

impl Scene {
    fn add_node(slf: &Bound<Self>, node: Node) -> PyResult<Py<Node>> {
        let mut slf_brw = slf.borrow_mut();
        let node = Py::new(slf.py(), node)?;
        slf_brw.nodes.push(node.clone_ref(slf.py()));
        Ok(node)
    }
}

#[pymethods]
impl Scene {
    #[new]
    pub fn __new__() -> Self {
        Self::default()
    }

    #[pyo3(signature = (name, *, extras = None))]
    pub fn create_root_node(slf: Bound<Self>, name: String, extras: Option<Dict>) -> PyResult<Py<Node>> {
        Self::add_node(
            &slf,
            Node {
                name,
                translation: None,
                rotation: None,
                scale: None,
                extras,
                parent: None,
                scene: slf.clone().unbind(),
            },
        )
    }

    #[pyo3(signature = (name, *, extras = None))]
    pub fn create_anim(slf: Bound<Self>, name: String, extras: Option<Dict>) -> PyResult<Py<Anim>> {
        let anim = Py::new(
            slf.py(),
            Anim {
                name,
                extras,
                nodes: HashMap::default(),
                scene: slf.clone().unbind(),
            },
        )?;
        slf.borrow_mut().anims.push(anim.clone_ref(slf.py()));
        Ok(anim)
    }

    pub fn export_glb(slf: Bound<Self>, path: PathBuf) -> PyResult<()> {
        let blob = export_glb(slf)?;
        write(path, blob)?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (angles, *, times = vec![], animation = None, include = None, exclude = None, background = None, output_dir = None))]
    pub fn render(
        slf: Bound<Self>,
        angles: Vec<CameraAngle>,
        times: Vec<f64>,
        animation: Option<String>,
        include: Option<Vec<String>>,
        exclude: Option<Vec<String>>,
        background: Option<(u8, u8, u8)>,
        output_dir: Option<PathBuf>,
    ) -> PyResult<RenderOutput> {
        render(slf, angles, times, animation, include, exclude, background, output_dir)
    }

    fn __traverse__(&self, visit: PyVisit) -> Result<(), PyTraverseError> {
        self.anims.iter().try_for_each(|anim| visit.call(anim))?;
        self.nodes.iter().try_for_each(|node| visit.call(node))?;
        self.meshes.iter().try_for_each(|model| visit.call(model))
    }

    fn __clear__(&mut self) {
        self.anims.clear();
        self.nodes.clear();
        self.meshes.clear();
    }
}

#[pyclass(frozen)]
pub struct Node {
    #[pyo3(get)]
    pub name: String,
    pub translation: Option<Vec3>,
    pub rotation: Option<Quat>,
    pub scale: Option<Vec3>,
    #[pyo3(get)]
    pub extras: Option<Dict>,
    #[pyo3(get)]
    pub parent: Option<Py<Node>>,
    #[pyo3(get)]
    pub scene: Py<Scene>,
}

#[pymethods]
impl Node {
    #[pyo3(signature = (name, *, translation = None, rotation = None, scale = None, extras = None))]
    pub fn create_child_node(
        slf: Bound<Self>,
        name: String,
        translation: Option<Vec3>,
        rotation: Option<Quat>,
        scale: Option<Vec3>,
        extras: Option<Dict>,
    ) -> PyResult<Py<Node>> {
        let scene = slf.get().scene.bind(slf.py());
        Scene::add_node(
            scene,
            Node {
                name,
                translation,
                rotation,
                scale,
                extras,
                parent: Some(slf.clone().unbind()),
                scene: scene.clone().unbind(),
            },
        )
    }

    #[pyo3(signature = (name, model, *, extras = None))]
    pub fn add_model(slf: Bound<Self>, name: String, model: Py<Model>, extras: Option<Dict>) -> PyResult<Py<Mesh>> {
        let mesh = Py::new(
            slf.py(),
            Mesh {
                name: name.clone(),
                extras,
                parent: slf.clone().unbind(),
                model,
            },
        )?;
        let mut scene = slf.get().scene.borrow_mut(slf.py());
        scene.meshes.push(mesh.clone_ref(slf.py()));
        Ok(mesh)
    }

    fn __traverse__(&self, visit: PyVisit) -> Result<(), PyTraverseError> {
        visit.call(&self.parent)?;
        visit.call(&self.scene)
    }
}

#[pyclass]
pub struct Mesh {
    #[pyo3(get)]
    pub name: String,
    #[pyo3(get)]
    pub extras: Option<Dict>,
    #[pyo3(get)]
    pub parent: Py<Node>,
    #[pyo3(get)]
    pub model: Py<Model>,
}

#[pymethods]
impl Mesh {
    fn __traverse__(&self, visit: PyVisit) -> Result<(), PyTraverseError> {
        visit.call(&self.parent)?;
        visit.call(&self.model)
    }
}
