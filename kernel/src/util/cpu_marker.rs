use core::iter::FusedIterator;

use crate::uses::*;
use crate::util::IMutex;
use crate::config::*;

#[derive(Debug)]
pub struct CpuMarker {
	marks: IMutex<[bool; MAX_CPUS]>,
}

impl CpuMarker {
	pub const fn new() -> Self {
		CpuMarker {
			marks: IMutex::new([false; MAX_CPUS]),
		}
	}

	pub fn mark(&self) {
		self.marks.lock()[prid()] = true;
	}

	pub fn unmark(&self) {
		self.marks.lock()[prid()] = false;
	}

	pub fn set(&self, mark: bool) {
		self.marks.lock()[prid()] = mark;
	}

	pub fn iter(&self) -> CpuMarkerIter {
		CpuMarkerIter::from(*self.marks.lock())
	}

	pub fn iter_clear(&self) -> CpuMarkerIter {
		let marks = self.marks.lock();
		let out = CpuMarkerIter::from(*marks);
		marks.map(|_| 0);
		out
	}
}

impl IntoIterator for CpuMarker {
	type Item = usize;
	type IntoIter = CpuMarkerIter;

	fn into_iter(self) -> Self::IntoIter {
		self.iter()
	}
}

pub struct CpuMarkerIter {
	marks: [bool; MAX_CPUS],
	index: usize,
}

impl CpuMarkerIter {
	fn from(marks: [bool; MAX_CPUS]) -> Self {
		CpuMarkerIter {
			marks,
			index: 0,
		}
	}
}

impl Iterator for CpuMarkerIter {
	type Item = usize;

	fn next(&mut self) -> Option<Self::Item> {
		for (i, elem) in self.marks[self.index..].iter().enumerate() {
			if *elem {
				self.index = i + 1;
				return Some(i);
			}
		}
		
		None
	}
}

impl FusedIterator for CpuMarkerIter {}
