#[derive(Eq, PartialEq, Hash, Clone, Debug)]
pub(crate) enum Side { Left, Right }

impl Side {
    pub(crate) fn opposite(&self) -> Self {
        match self {
            Side::Left => Side::Right,
            Side::Right => Side::Left,
        }
    }

    pub(crate) fn values() -> impl Iterator<Item = &'static Side> {
        static VALUES: [Side; 2] = [Side::Left, Side::Right];

        VALUES.iter()
    }

    pub(crate) fn is_right(&self) -> bool {
        self == &Side::Right
    }
}
