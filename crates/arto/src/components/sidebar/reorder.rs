//! Dragging a row of a saved list to a new position.

use std::path::PathBuf;

/// One row being dragged, or one being rested on: where it is drawn, and what
/// it is.
///
/// The position travels with the path because it is what says which way the
/// dragged row is going, and that decides the side it lands on.
pub type DragRow = (usize, PathBuf);

/// Which side of `row` the dragged row would land on, or `None` if this is not
/// the row being rested on.
///
/// The direction of travel decides it: a row dragged downwards lands after
/// what it is over, one dragged upwards before it. Every position is then
/// reachable — landing only ever before a row leaves nothing that can be made
/// last — and the line does not jump as the pointer crosses the middle of a
/// row.
pub fn drop_side(dragging: &Option<DragRow>, target: &Option<DragRow>, row: usize) -> Option<bool> {
    let (from, _) = dragging.as_ref()?;
    let (to, _) = target.as_ref()?;
    (*to == row && from != to).then_some(from < to)
}

/// The class a row carries while it is the one a drag would land beside.
pub fn drop_class(side: Option<bool>) -> &'static str {
    match side {
        Some(true) => "left-sidebar-drop-after",
        Some(false) => "left-sidebar-drop-before",
        None => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(index: usize) -> Option<DragRow> {
        Some((index, PathBuf::from(format!("/{index}"))))
    }

    #[test]
    fn dragging_downwards_lands_after_the_row_it_is_over() {
        assert_eq!(drop_side(&row(0), &row(2), 2), Some(true));
    }

    #[test]
    fn dragging_upwards_lands_before_the_row_it_is_over() {
        assert_eq!(drop_side(&row(3), &row(1), 1), Some(false));
    }

    #[test]
    fn a_row_that_is_not_being_rested_on_gets_no_line() {
        assert_eq!(drop_side(&row(0), &row(2), 1), None);
    }

    #[test]
    fn a_row_resting_on_itself_gets_no_line() {
        assert_eq!(drop_side(&row(2), &row(2), 2), None);
    }

    #[test]
    fn nothing_is_marked_while_nothing_is_being_dragged() {
        assert_eq!(drop_side(&None, &row(2), 2), None);
        assert_eq!(drop_side(&row(0), &None, 0), None);
    }
}
