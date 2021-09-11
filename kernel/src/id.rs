#[macro_export]
macro_rules! make_id_type {
	($type:ident, $int_type:ident) => {
		#[repr(transparent)]
		#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
		pub struct $type($int_type);

		impl $type {
			pub fn from(id: $int_type) -> Self {
				Self(id)
			}

			pub fn into(self) -> $int_type {
				self.0
			}
		}
	};

	($type:tt) => {
		$crate::make_id_type!($type, usize);
	};
}
