use failure::Error;
use std::marker::PhantomData;
use std::ops::{Add, Deref, DerefMut, Mul};

use geometry::Geometry;
use geometry::convert::AsPosition;
use graph::{GraphError, Perimeter};
use graph::geometry::{EdgeLateral, EdgeMidpoint};
use graph::geometry::alias::{ScaledEdgeLateral, VertexPosition};
use graph::mesh::{Edge, Mesh};
use graph::storage::{EdgeKey, VertexKey};
use graph::topology::{FaceView, OrphanFaceView, OrphanVertexView, OrphanView, Topological,
                      VertexView, View};

/// Do **not** use this type directly. Use `EdgeRef` and `EdgeMut` instead.
///
/// This type is only re-exported so that its members are shown in
/// documentation. See this issue:
/// https://github.com/rust-lang/rust/issues/39437
pub struct EdgeView<M, G>
where
    M: AsRef<Mesh<G>>,
    G: Geometry,
{
    mesh: M,
    key: EdgeKey,
    phantom: PhantomData<G>,
}

impl<M, G> EdgeView<M, G>
where
    M: AsRef<Mesh<G>>,
    G: Geometry,
{
    pub(crate) fn new(mesh: M, edge: EdgeKey) -> Self {
        EdgeView {
            mesh: mesh,
            key: edge,
            phantom: PhantomData,
        }
    }

    pub fn key(&self) -> EdgeKey {
        self.key
    }

    pub fn to_key_topology(&self) -> EdgeKeyTopology {
        EdgeKeyTopology::new(self.key, self.key.to_vertex_keys())
    }

    pub fn source_vertex(&self) -> VertexView<&Mesh<G>, G> {
        let (vertex, _) = self.key.to_vertex_keys();
        VertexView::new(self.mesh.as_ref(), vertex)
    }

    pub fn into_source_vertex(self) -> VertexView<M, G> {
        let (vertex, _) = self.key.to_vertex_keys();
        let mesh = self.mesh;
        VertexView::new(mesh, vertex)
    }

    pub fn destination_vertex(&self) -> VertexView<&Mesh<G>, G> {
        VertexView::new(self.mesh.as_ref(), self.vertex)
    }

    pub fn into_destination_vertex(self) -> VertexView<M, G> {
        let vertex = self.vertex;
        let mesh = self.mesh;
        VertexView::new(mesh, vertex)
    }

    pub fn opposite_edge(&self) -> Option<EdgeView<&Mesh<G>, G>> {
        self.opposite
            .map(|opposite| EdgeView::new(self.mesh.as_ref(), opposite))
    }

    pub fn into_opposite_edge(self) -> Option<Self> {
        let opposite = self.opposite;
        let mesh = self.mesh;
        opposite.map(|opposite| EdgeView::new(mesh, opposite))
    }

    pub fn next_edge(&self) -> Option<EdgeView<&Mesh<G>, G>> {
        self.next
            .map(|next| EdgeView::new(self.mesh.as_ref(), next))
    }

    pub fn into_next_edge(self) -> Option<Self> {
        let next = self.next;
        let mesh = self.mesh;
        next.map(|next| EdgeView::new(mesh, next))
    }

    pub fn face(&self) -> Option<FaceView<&Mesh<G>, G>> {
        self.face
            .map(|face| FaceView::new(self.mesh.as_ref(), face))
    }

    pub fn into_face(self) -> Option<FaceView<M, G>> {
        let face = self.face;
        let mesh = self.mesh;
        face.map(|face| FaceView::new(mesh, face))
    }

    // Resolve the `M` parameter to a concrete reference.
    #[allow(dead_code)]
    fn with_mesh_ref(&self) -> EdgeView<&Mesh<G>, G> {
        EdgeView::new(self.mesh.as_ref(), self.key)
    }
}

impl<M, G> EdgeView<M, G>
where
    M: AsRef<Mesh<G>> + AsMut<Mesh<G>>,
    G: Geometry,
{
    pub fn opposite_edge_mut(&mut self) -> Option<OrphanEdgeView<G>> {
        let opposite = self.opposite;
        opposite.map(move |opposite| {
            OrphanEdgeView::new(
                self.mesh.as_mut().edges.get_mut(&opposite).unwrap(),
                opposite,
            )
        })
    }

    pub fn next_edge_mut(&mut self) -> Option<OrphanEdgeView<G>> {
        let next = self.next;
        next.map(move |next| {
            OrphanEdgeView::new(self.mesh.as_mut().edges.get_mut(&next).unwrap(), next)
        })
    }

    pub fn source_vertex_mut(&mut self) -> OrphanVertexView<G> {
        let (vertex, _) = self.key().to_vertex_keys();
        OrphanVertexView::new(
            self.mesh.as_mut().vertices.get_mut(&vertex).unwrap(),
            vertex,
        )
    }

    pub fn destination_vertex_mut(&mut self) -> OrphanVertexView<G> {
        let vertex = self.vertex;
        OrphanVertexView::new(
            self.mesh.as_mut().vertices.get_mut(&vertex).unwrap(),
            vertex,
        )
    }

    pub fn face_mut(&mut self) -> Option<OrphanFaceView<G>> {
        let face = self.face;
        face.map(move |face| {
            OrphanFaceView::new(self.mesh.as_mut().faces.get_mut(&face).unwrap(), face)
        })
    }

    pub fn join(mut self, edge: EdgeKey) -> Result<Self, Error> {
        if self.mesh.as_ref().edges.get(&edge).is_none() {
            return Err(GraphError::TopologyNotFound.into());
        }
        let (a, b) = self.key().to_vertex_keys();
        let (c, d) = edge.to_vertex_keys();
        // At this point, we can assume the points a, b, c, and d exist in the
        // mesh. Before mutating the mesh, ensure that there are no edges
        // connecting their interior.
        for ab in [d, c, b, a].perimeter() {
            if self.mesh.as_ref().edges.get(&ab.into()).is_some() {
                return Err(GraphError::TopologyConflict.into());
            }
        }
        // Insert the edges and faces (two triangles forming a quad). These
        // operations should not fail; unwrap their results.
        let extrusion = {
            let edge = self.geometry.clone();
            let face = self.face()
                .ok_or(Error::from(GraphError::TopologyNotFound))?
                .geometry
                .clone();
            let mesh = self.mesh.as_mut();
            // Triangle of b-a-d.
            let ba = mesh.insert_edge((b, a), edge.clone()).unwrap();
            let ad = mesh.insert_edge((a, d), edge.clone()).unwrap();
            let db = mesh.insert_edge((d, b), edge.clone()).unwrap();
            // Triangle of b-d-c.
            let bd = mesh.insert_edge((b, d), edge.clone()).unwrap();
            let dc = mesh.insert_edge((d, c), edge.clone()).unwrap();
            let cb = mesh.insert_edge((c, b), edge).unwrap();
            mesh.insert_face(&[ba, ad, db], face.clone()).unwrap();
            mesh.insert_face(&[bd, dc, cb], face).unwrap();
            dc
        };
        Ok(EdgeView::new(self.mesh, extrusion))
    }

    // Resolve the `M` parameter to a concrete reference.
    #[allow(dead_code)]
    fn with_mesh_mut(&mut self) -> EdgeView<&mut Mesh<G>, G> {
        EdgeView::new(self.mesh.as_mut(), self.key)
    }
}

impl<M, G> EdgeView<M, G>
where
    M: AsRef<Mesh<G>>,
    G: EdgeMidpoint + Geometry,
{
    pub fn midpoint(&self) -> Result<G::Midpoint, Error> {
        G::midpoint(self.with_mesh_ref())
    }
}

impl<M, G> EdgeView<M, G>
where
    M: AsRef<Mesh<G>> + AsMut<Mesh<G>>,
    G: EdgeMidpoint + Geometry,
    G::Vertex: AsPosition,
{
    pub fn split(mut self) -> Result<VertexView<M, G>, Error>
    where
        G: EdgeMidpoint<Midpoint = VertexPosition<G>>,
    {
        // Insert a new vertex at the midpoint.
        let m = {
            let mut m = self.source_vertex().geometry.clone();
            *m.as_position_mut() = self.midpoint()?;
            // This is the point of no return; the mesh has been mutated.
            self.mesh.as_mut().insert_vertex(m)
        };
        // Get both half-edges to be split.
        let edge = self.key();
        let opposite = self.opposite_edge().map(|opposite| opposite.key());
        let mut mesh = self.mesh;
        // Split the half-edges. This should not fail; unwrap the results.
        Self::split_half_at(&mut mesh, edge, m).unwrap();
        if let Some(opposite) = opposite {
            Self::split_half_at(&mut mesh, opposite, m).unwrap();
        }
        Ok(VertexView::new(mesh, m))
    }

    fn split_half_at(
        mesh: &mut M,
        edge: EdgeKey,
        m: VertexKey,
    ) -> Result<(EdgeKey, EdgeKey), Error> {
        // Remove the edge and insert two truncated edges in its place.
        let source = mesh.as_mut().edges.remove(&edge).unwrap();
        let (a, b) = edge.to_vertex_keys();
        let am = mesh.as_mut().insert_edge((a, m), source.geometry.clone())?;
        let mb = mesh.as_mut().insert_edge((m, b), source.geometry.clone())?;
        // Connect the new edges to each other and their leading edges.
        {
            let edge = mesh.as_mut().edges.get_mut(&am).unwrap();
            edge.next = Some(mb);
            edge.previous = source.previous;
            edge.face = source.face
        }
        {
            let edge = mesh.as_mut().edges.get_mut(&mb).unwrap();
            edge.next = source.next;
            edge.previous = Some(am);
            edge.face = source.face;
        }
        if let Some(pa) = source.previous {
            mesh.as_mut().edges.get_mut(&pa).unwrap().next = Some(am);
        }
        if let Some(bn) = source.next {
            mesh.as_mut().edges.get_mut(&bn).unwrap().previous = Some(mb);
        }
        // Update the associated face, if any, because it may refer to the
        // removed edge.
        if let Some(face) = source.face {
            mesh.as_mut().faces.get_mut(&face).unwrap().edge = am;
        }
        Ok((am, mb))
    }
}

impl<M, G> EdgeView<M, G>
where
    M: AsRef<Mesh<G>>,
    G: Geometry + EdgeLateral,
{
    pub fn lateral(&self) -> Result<G::Lateral, Error> {
        G::lateral(self.with_mesh_ref())
    }
}

impl<M, G> EdgeView<M, G>
where
    M: AsRef<Mesh<G>> + AsMut<Mesh<G>>,
    G: Geometry + EdgeLateral,
    G::Vertex: AsPosition,
{
    pub fn extrude<T>(mut self, distance: T) -> Result<Self, Error>
    where
        G::Lateral: Mul<T>,
        ScaledEdgeLateral<G, T>: Clone,
        VertexPosition<G>: Add<ScaledEdgeLateral<G, T>, Output = VertexPosition<G>> + Clone,
    {
        let face = self.face()
            .ok_or(Error::from(GraphError::TopologyNotFound))?
            .geometry
            .clone();
        // Insert new vertices with the specified translation and get all
        // vertex keys.
        let (a, b, c, d) = {
            // Get the originating vertices and their geometry.
            let (a, mut d, b, mut c) = {
                let a = self.source_vertex();
                let b = self.destination_vertex();
                (a.key(), a.geometry.clone(), b.key(), b.geometry.clone())
            };
            // Clone the geometry and translate it using the lateral normal,
            // then insert the new vertex geometry and yield the vertex keys.
            let translation = self.lateral()? * distance;
            *c.as_position_mut() = c.as_position().clone() + translation.clone();
            *d.as_position_mut() = d.as_position().clone() + translation;
            (
                a,
                b,
                // This is the point of no return; the mesh has been mutated.
                self.mesh.as_mut().insert_vertex(c),
                self.mesh.as_mut().insert_vertex(d),
            )
        };
        // Insert the edges and faces (two triangles forming a quad) and get
        // the extruded edge's key. These operations should not fail; unwrap
        // their results.
        let extrusion = {
            let edge = self.geometry.clone();
            let mesh = self.mesh.as_mut();
            // Triangle of b-a-d.
            let ba = mesh.insert_edge((b, a), edge.clone()).unwrap();
            let ad = mesh.insert_edge((a, d), edge.clone()).unwrap();
            let db = mesh.insert_edge((d, b), edge.clone()).unwrap();
            // Triangle of b-d-c.
            let bd = mesh.insert_edge((b, d), edge.clone()).unwrap();
            let dc = mesh.insert_edge((d, c), edge.clone()).unwrap();
            let cb = mesh.insert_edge((c, b), edge).unwrap();
            mesh.insert_face(&[ba, ad, db], face.clone()).unwrap();
            mesh.insert_face(&[bd, dc, cb], face).unwrap();
            dc
        };
        Ok(EdgeView::new(self.mesh, extrusion))
    }
}

impl<M, G> AsRef<EdgeView<M, G>> for EdgeView<M, G>
where
    M: AsRef<Mesh<G>>,
    G: Geometry,
{
    fn as_ref(&self) -> &EdgeView<M, G> {
        self
    }
}

impl<M, G> AsMut<EdgeView<M, G>> for EdgeView<M, G>
where
    M: AsRef<Mesh<G>> + AsMut<Mesh<G>>,
    G: Geometry,
{
    fn as_mut(&mut self) -> &mut EdgeView<M, G> {
        self
    }
}

impl<M, G> Deref for EdgeView<M, G>
where
    M: AsRef<Mesh<G>>,
    G: Geometry,
{
    type Target = Edge<G>;

    fn deref(&self) -> &Self::Target {
        self.mesh.as_ref().edges.get(&self.key).unwrap()
    }
}

impl<M, G> DerefMut for EdgeView<M, G>
where
    M: AsRef<Mesh<G>> + AsMut<Mesh<G>>,
    G: Geometry,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.mesh.as_mut().edges.get_mut(&self.key).unwrap()
    }
}

impl<M, G> Clone for EdgeView<M, G>
where
    M: AsRef<Mesh<G>> + Clone,
    G: Geometry,
{
    fn clone(&self) -> Self {
        EdgeView {
            mesh: self.mesh.clone(),
            key: self.key.clone(),
            phantom: PhantomData,
        }
    }
}

impl<M, G> Copy for EdgeView<M, G>
where
    M: AsRef<Mesh<G>> + Copy,
    G: Geometry,
{
}

impl<M, G> View<M, G> for EdgeView<M, G>
where
    M: AsRef<Mesh<G>>,
    G: Geometry,
{
    type Topology = Edge<G>;

    fn from_mesh(mesh: M, key: <Self::Topology as Topological>::Key) -> Self {
        EdgeView::new(mesh, key)
    }
}

/// Do **not** use this type directly. Use `OrphanEdgeMut` instead.
///
/// This type is only re-exported so that its members are shown in
/// documentation. See this issue:
/// https://github.com/rust-lang/rust/issues/39437
pub struct OrphanEdgeView<'a, G>
where
    G: 'a + Geometry,
{
    key: EdgeKey,
    edge: &'a mut Edge<G>,
}

impl<'a, G> OrphanEdgeView<'a, G>
where
    G: 'a + Geometry,
{
    pub(crate) fn new(edge: &'a mut Edge<G>, key: EdgeKey) -> Self {
        OrphanEdgeView {
            key: key,
            edge: edge,
        }
    }

    pub fn key(&self) -> EdgeKey {
        self.key
    }
}

impl<'a, G> Deref for OrphanEdgeView<'a, G>
where
    G: 'a + Geometry,
{
    type Target = <Self as OrphanView<'a, G>>::Topology;

    fn deref(&self) -> &Self::Target {
        &*self.edge
    }
}

impl<'a, G> DerefMut for OrphanEdgeView<'a, G>
where
    G: 'a + Geometry,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.edge
    }
}

impl<'a, G> OrphanView<'a, G> for OrphanEdgeView<'a, G>
where
    G: 'a + Geometry,
{
    type Topology = Edge<G>;

    fn from_topology(
        topology: &'a mut Self::Topology,
        key: <Self::Topology as Topological>::Key,
    ) -> Self {
        OrphanEdgeView::new(topology, key)
    }
}

pub struct EdgeKeyTopology {
    key: EdgeKey,
    vertices: (VertexKey, VertexKey),
}

impl EdgeKeyTopology {
    fn new(edge: EdgeKey, vertices: (VertexKey, VertexKey)) -> Self {
        EdgeKeyTopology {
            key: edge,
            vertices: vertices,
        }
    }

    pub fn key(&self) -> EdgeKey {
        self.key
    }

    pub fn vertices(&self) -> (VertexKey, VertexKey) {
        self.vertices
    }
}

#[cfg(test)]
mod tests {
    use nalgebra::Point3;

    use generate::*;
    use graph::*;
    use graph::storage::Key;

    #[test]
    fn extrude_edge() {
        let mut mesh = Mesh::<Point3<f32>>::from_raw_buffers(
            vec![0, 1, 2, 3],
            vec![
                (0.0, 0.0, 0.0),
                (1.0, 0.0, 0.0),
                (1.0, 1.0, 0.0),
                (0.0, 1.0, 0.0),
            ],
            4,
        ).unwrap();
        let key = mesh.edges().nth(0).unwrap().key();
        mesh.edge_mut(key).unwrap().extrude(1.0).unwrap();

        assert_eq!(10, mesh.edge_count());
        assert_eq!(3, mesh.face_count());
    }

    #[test]
    fn join_edges() {
        // Construct a mesh with two independent quads.
        let mut mesh = Mesh::<Point3<f32>>::from_raw_buffers(
            vec![0, 1, 2, 3, 4, 5, 6, 7],
            vec![
                (-2.0, 0.0, 0.0),
                (-1.0, 0.0, 0.0), // 1
                (-1.0, 1.0, 0.0), // 2
                (-2.0, 1.0, 0.0),
                (1.0, 0.0, 0.0), // 4
                (2.0, 0.0, 0.0),
                (2.0, 1.0, 0.0),
                (1.0, 1.0, 0.0), // 7
            ],
            4,
        ).unwrap();
        // TODO: This is fragile. It would probably be best for `Mesh` to
        //       provide a more convenient way to search for topology.
        // Construct the keys for the nearby edges.
        let source = (VertexKey::from(Key::new(1)), VertexKey::from(Key::new(2))).into();
        let destination = (VertexKey::from(Key::new(7)), VertexKey::from(Key::new(4))).into();
        mesh.edge_mut(source).unwrap().join(destination).unwrap();

        assert_eq!(14, mesh.edge_count());
        assert_eq!(4, mesh.face_count());
    }

    #[test]
    fn split_half_edge() {
        let mut mesh = Mesh::<Point3<f32>>::from_raw_buffers(
            vec![0, 1, 2, 3],
            vec![
                (0.0, 0.0, 0.0),
                (1.0, 0.0, 0.0),
                (1.0, 1.0, 0.0),
                (0.0, 1.0, 0.0),
            ],
            4,
        ).unwrap();
        let key = mesh.edges().nth(0).unwrap().key();
        let vertex = mesh.edge_mut(key).unwrap().split().unwrap();

        assert_eq!(
            5,
            vertex
                .outgoing_edge()
                .unwrap()
                .face()
                .unwrap()
                .edges()
                .count()
        );
    }

    #[test]
    fn split_full_edge() {
        let (indeces, vertices) = cube::Cube::new()
            .polygons_with_position() // 6 quads, 24 vertices.
            .flat_index_vertices(HashIndexer::default());
        let mut mesh = Mesh::<Point3<f32>>::from_raw_buffers(indeces, vertices, 4).unwrap();
        let key = mesh.edges().nth(0).unwrap().key();
        let vertex = mesh.edge_mut(key).unwrap().split().unwrap();

        assert_eq!(
            5,
            vertex
                .outgoing_edge()
                .unwrap()
                .face()
                .unwrap()
                .edges()
                .count()
        );
        assert_eq!(
            5,
            vertex
                .outgoing_edge()
                .unwrap()
                .opposite_edge()
                .unwrap()
                .face()
                .unwrap()
                .edges()
                .count()
        );
    }
}
