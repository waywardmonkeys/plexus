use graph::geometry::{Attribute, Geometry};
use graph::storage::{EdgeKey, FaceKey, Key, Storage, VertexKey};

#[derive(Clone, Debug)]
pub struct Vertex<T, K>
where
    T: Attribute,
    K: Key,
{
    pub geometry: T,
    pub(super) edge: Option<EdgeKey<K>>,
}

#[derive(Clone, Debug)]
pub struct Edge<T, K>
where
    T: Attribute,
    K: Key,
{
    pub geometry: T,
    pub(super) opposite: Option<EdgeKey<K>>,
    pub(super) next: Option<EdgeKey<K>>,
    pub(super) vertex: VertexKey<K>,
}

#[derive(Clone, Debug)]
pub struct Face<T, K>
where
    T: Attribute,
    K: Key,
{
    pub geometry: T,
    pub(super) edge: EdgeKey<K>,
}

pub struct Mesh<G, K = u64>
where
    G: Geometry,
    K: Key,
{
    pub(super) vertices: Storage<K, Vertex<G::Vertex, K>>,
    pub(super) edges: Storage<K, Edge<G::Edge, K>>,
    pub(super) faces: Storage<K, Face<G::Face, K>>,
}

impl<G, K> Mesh<G, K>
where
    G: Geometry,
    K: Key,
{
    pub fn new() -> Self {
        Mesh {
            vertices: Storage::new(),
            edges: Storage::new(),
            faces: Storage::new(),
        }
    }

    pub(crate) fn insert_vertex(&mut self, geometry: G::Vertex) -> VertexKey<K> {
        let vertex = Vertex {
            geometry: geometry,
            edge: None,
        };
        self.vertices.insert(vertex).into()
    }
}

impl<G, K> AsRef<Mesh<G, K>> for Mesh<G, K>
where
    G: Geometry,
    K: Key,
{
    fn as_ref(&self) -> &Self {
        self
    }
}

impl<G, K> AsMut<Mesh<G, K>> for Mesh<G, K>
where
    G: Geometry,
    K: Key,
{
    fn as_mut(&mut self) -> &mut Self {
        self
    }
}
