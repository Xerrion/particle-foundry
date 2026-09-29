//! Session ownership checks for the CPU transport inventory.

use particle_sim::Grid;
use particle_sim_cpu::{
    ReferenceSession,
    transport::{CELL_VOLUME_M3, TransportError, TransportInventory},
};

fn carrier_inventory(grid: Grid) -> TransportInventory {
    TransportInventory::new(
        grid,
        1000.0,
        1.2,
        vec![0.0; grid.cells()],
        vec![1.2 * CELL_VOLUME_M3; grid.cells()],
        vec![0.0; grid.cells()],
        vec![0.0; grid.cells()],
        vec![false; grid.cells()],
    )
    .unwrap()
}

#[test]
fn session_owns_one_validated_inventory_and_rejects_other_geometry() {
    let grid = Grid::new(2.0, 1.0).unwrap();
    let mut session = ReferenceSession::new(grid);
    let inventory = carrier_inventory(grid);
    let expected = inventory.clone();
    session.set_transport_inventory(inventory).unwrap();
    assert_eq!(session.transport_inventory(), Some(&expected));

    let other = carrier_inventory(Grid::new(1.0, 2.0).unwrap());
    assert_eq!(
        session.set_transport_inventory(other),
        Err(TransportError::GridMismatch)
    );
    assert_eq!(session.transport_inventory(), Some(&expected));
}
