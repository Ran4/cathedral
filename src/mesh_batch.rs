//! Shared geometry for the batches that are rebuilt from scratch every frame:
//! rain streaks, splash rings, chimney puffs. Each is a single mesh asset
//! holding every camera-facing quad of its kind, rewritten in place each tick.
//!
//! An idle batch — a dry sky, a city of doused hearths — parks on one zero-area
//! triangle rather than on nothing at all. Bevy's mesh allocator skips a
//! zero-vertex mesh while it stages slab allocations but still tries to copy
//! that mesh's data in afterwards, so every extraction of an empty batch logged
//! two `Use-after-free: attempted to copy element data for an unallocated key`
//! errors — once for the vertex buffer, once for the index buffer, every frame
//! the batch had nothing to show. The stand-in triangle allocates normally,
//! rasterizes no fragments, and costs three vertices.

use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};

/// Vertices in the stand-in triangle an idle batch holds. Live batches are
/// built from quads, so a real frame is always a multiple of four and never
/// collides with this count.
pub const IDLE_BATCH_VERTICES: usize = 3;

/// A batch mesh carrying one frame of quads, or the idle triangle when the
/// frame produced none.
pub fn batch_mesh(
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    colors: Vec<[f32; 4]>,
    indices: Vec<u32>,
) -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    fill(&mut mesh, positions, normals, uvs, colors, indices);
    mesh
}

/// A batch mesh with every attribute present and nothing visible in it — what a
/// batch is spawned with, before its first frame of geometry.
pub fn idle_batch_mesh() -> Mesh {
    batch_mesh(Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new())
}

/// Replaces a batch mesh's geometry with one frame of quads, parking it on the
/// idle triangle when the frame produced none. A batch that is already parked
/// is left untouched: marking the asset modified would re-extract and re-upload
/// it every frame for no visible change.
pub fn write_batch_mesh(
    meshes: &mut Assets<Mesh>,
    handle: &Handle<Mesh>,
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    colors: Vec<[f32; 4]>,
    indices: Vec<u32>,
) {
    if positions.is_empty()
        && meshes
            .get(handle)
            .is_some_and(|mesh| mesh.count_vertices() == IDLE_BATCH_VERTICES)
    {
        return;
    }
    let Some(mut mesh) = meshes.get_mut(handle) else {
        return;
    };
    fill(&mut mesh, positions, normals, uvs, colors, indices);
}

fn fill(
    mesh: &mut Mesh,
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    colors: Vec<[f32; 4]>,
    indices: Vec<u32>,
) {
    if positions.is_empty() {
        // Three coincident, fully transparent vertices: one allocated slot in
        // the mesh slabs, zero covered pixels.
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_POSITION,
            vec![[0.0; 3]; IDLE_BATCH_VERTICES],
        );
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_NORMAL,
            vec![[0.0, 1.0, 0.0]; IDLE_BATCH_VERTICES],
        );
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0; 2]; IDLE_BATCH_VERTICES]);
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, vec![[0.0; 4]; IDLE_BATCH_VERTICES]);
        mesh.insert_indices(Indices::U32((0..IDLE_BATCH_VERTICES as u32).collect()));
        return;
    }
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));
}

#[cfg(test)]
mod tests {
    use bevy::asset::AssetPlugin;

    use super::*;

    fn app_with_assets() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Mesh>();
        app
    }

    /// Runs a frame and reports how many times the mesh assets were rewritten
    /// during it — the work an idle batch must not cost.
    fn modifications(app: &mut App) -> usize {
        app.update();
        app.world_mut()
            .resource_mut::<Messages<AssetEvent<Mesh>>>()
            .drain()
            .filter(|event| matches!(event, AssetEvent::Modified { .. }))
            .count()
    }

    /// One frame of `quads` quads into the batch; zero is an idle frame.
    fn write_frame(app: &mut App, handle: &Handle<Mesh>, quads: usize) {
        let mut positions: Vec<[f32; 3]> = Vec::new();
        let mut normals: Vec<[f32; 3]> = Vec::new();
        let mut uvs: Vec<[f32; 2]> = Vec::new();
        let mut colors: Vec<[f32; 4]> = Vec::new();
        let mut indices: Vec<u32> = Vec::new();
        for quad in 0..quads {
            let first = positions.len() as u32;
            let y = quad as f32;
            positions.extend([
                [0.0, y, 0.0],
                [1.0, y, 0.0],
                [1.0, y + 1.0, 0.0],
                [0.0, y + 1.0, 0.0],
            ]);
            normals.extend([[0.0, 0.0, 1.0]; 4]);
            uvs.extend([[0.0; 2]; 4]);
            colors.extend([[1.0; 4]; 4]);
            indices.extend([first, first + 1, first + 2, first, first + 2, first + 3]);
        }
        let mut meshes = app.world_mut().resource_mut::<Assets<Mesh>>();
        write_batch_mesh(
            &mut meshes,
            handle,
            positions,
            normals,
            uvs,
            colors,
            indices,
        );
    }

    fn vertices(app: &App, handle: &Handle<Mesh>) -> usize {
        app.world()
            .resource::<Assets<Mesh>>()
            .get(handle)
            .expect("the batch mesh asset exists")
            .count_vertices()
    }

    /// The whole point: a batch mesh is never zero-vertex, because Bevy's mesh
    /// allocator never allocates a slab for one and then logs a use-after-free
    /// when it copies the data in anyway.
    #[test]
    fn an_idle_batch_is_drawable_rather_than_empty() {
        let mesh = idle_batch_mesh();
        assert_eq!(mesh.count_vertices(), IDLE_BATCH_VERTICES);
        assert!(mesh.indices().is_some_and(|indices| indices.len() == 3));
        for attribute in [
            Mesh::ATTRIBUTE_POSITION,
            Mesh::ATTRIBUTE_NORMAL,
            Mesh::ATTRIBUTE_UV_0,
            Mesh::ATTRIBUTE_COLOR,
        ] {
            assert!(
                mesh.attribute(attribute).is_some(),
                "an idle batch keeps every attribute the live layout has"
            );
        }
        // Zero area: the stand-in never covers a pixel.
        let positions = mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap();
        match positions {
            bevy::mesh::VertexAttributeValues::Float32x3(points) => {
                assert!(points.iter().all(|point| *point == points[0]));
            }
            other => panic!("unexpected position format: {other:?}"),
        }
    }

    /// A batch that runs out of geometry stays drawable, and once parked it
    /// costs nothing per frame: a dry sky must not re-upload a mesh every tick.
    #[test]
    fn a_batch_falls_back_to_the_idle_triangle_and_then_stays_put() {
        let mut app = app_with_assets();
        let handle = app
            .world_mut()
            .resource_mut::<Assets<Mesh>>()
            .add(idle_batch_mesh());
        modifications(&mut app);

        write_frame(&mut app, &handle, 1);
        assert_eq!(vertices(&app, &handle), 4);
        assert_eq!(modifications(&mut app), 1);

        write_frame(&mut app, &handle, 0);
        assert_eq!(vertices(&app, &handle), IDLE_BATCH_VERTICES);
        assert_eq!(modifications(&mut app), 1);

        write_frame(&mut app, &handle, 0);
        assert_eq!(vertices(&app, &handle), IDLE_BATCH_VERTICES);
        assert_eq!(
            modifications(&mut app),
            0,
            "an idle batch that stays idle must not touch its asset"
        );
    }
}
