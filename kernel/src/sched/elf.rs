use bitflags::bitflags;
use crate::uses::*;
use crate::mem::VirtRange;

#[derive(Debug)]
pub struct Section<'a>
{
	pub virt_range: VirtRange,
	pub data: &'a [u8],
	// data virtual offset from start of virt_range
	pub data_offset: usize,
	pub flags: PHdrFlags,
}

#[derive(Debug)]
pub struct ElfParser<'a>
{
	data: &'a [u8],
	elf_header: &'a ElfHeader,
	program_headers: &'a [ProgramHeader],
}

impl<'a> ElfParser<'a>
{
	pub fn new (data: &[u8]) -> Result<ElfParser, Err>
	{
		let elf_header = ElfHeader::new (data)?;
		elf_header.check ()?;

		let phdr = elf_header.program_header;
		let phdr_len = elf_header.phdr_len as usize;

		// TODO: figure out if it matters that alignment requirements might not be met
		// (probably not on x86)
		let program_headers = Self::extract_slice (data, phdr, phdr_len)
			.ok_or_else (|| Err::new ("invalid program headers"))?;

		Ok(ElfParser {
			data,
			elf_header,
			program_headers,
		})
	}

	pub fn program_headers (&self) -> Vec<Section>
	{
		let mut out = Vec::new ();
		for header in self.program_headers.iter ()
		{
			if header.ptype == P_TYPE_LOAD
			{
				if header.p_filesz == 0 || header.p_memsz == 0
				{
					continue;
				}

				let virt_range = VirtRange::new_unaligned (VirtAddr::new (header.p_vaddr as u64), header.p_memsz);
				let virt_range_aligned = virt_range.aligned ();
				let data = self.extract (header.p_offset, header.p_filesz);
				match data
				{
					Some(data) => {
						out.push (Section {
							virt_range: virt_range_aligned,
							data,
							data_offset: virt_range.as_usize () - virt_range_aligned.as_usize (),
							flags: header.flags,
						})
					},
					None => continue,
				}
			}
		}
		out
	}

	pub fn entry_point (&self) -> fn() -> ()
	{
		unsafe
		{
			core::mem::transmute (self.elf_header.entry)
		}
	}

	fn extract_slice<T> (data: &[u8], index: usize, len: usize) -> Option<&[T]>
	{
		let slice = data.get (index..(index + len * size_of::<T> ()))?;
		let ptr = slice.as_ptr () as *const T;
		unsafe
		{
			Some(core::slice::from_raw_parts (ptr, len))
		}
	}

	fn extract<T> (&self, index: usize, len: usize) -> Option<&[T]>
	{
		let slice = self.data.get (index..(index + len * size_of::<T> ()))?;
		let ptr = slice.as_ptr () as *const T;
		unsafe
		{
			Some(core::slice::from_raw_parts (ptr, len))
		}
	}
}

// NOTE: this only applies to little endian architectures
const ELF_MAGIC: u32 = 0x464c457f;

const BIT_32: u8 = 1;
const BIT_64: u8 = 2;

const LITTLE_ENDIAN: u8 = 1;
const BIG_ENDIAN: u8 = 2;

const SYSTEM_V_ABI: u8 = 0;

const RELOCATABLE: u16 = 1;
const EXECUTABLE: u16 = 2;
const SHARED: u16 = 3;
const CORE: u16 = 4;

const X64: u16 = 0x3e;

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
struct ElfHeader
{
	// elf magic number
	magic: u32,
	// 1: 32 bits
	// 2: 64 bits
	bits: u8,
	// 1: little endian
	// 2: big endian
	endianness: u8,
	header_version: u8,
	// 0 for system V
	abi: u8,
	unused: u64,
	// 1: relocatable
	// 2: executable
	// 3: shared
	// 4: core
	info: u16,
	arch: u16,
	elf_version: u32,
	entry: usize,
	program_header: usize,
	section_header: usize,
	flags: u32,
	header_size: u16,
	phdr_entry_size: u16,
	phdr_len: u16,
	shdr_entry_size: u16,
	shdr_len: u16,
	shdr_names_index: u16,
}

impl ElfHeader
{
	fn new (data: &[u8]) -> Result<&Self, Err>
	{
		if data.len () < size_of::<Self> ()
		{
			return Err(Err::new ("invalid elf header"));
		}
		unsafe
		{
			Ok((data.as_ptr () as *const Self).as_ref ().unwrap ())
		}
	}

	fn check (&self) -> Result<(), Err>
	{
		if self.magic != ELF_MAGIC
		{
			Err(Err::new ("Binary is not ELF"))
		}
		else if self.bits != BIT_64 || self.endianness != LITTLE_ENDIAN || self.arch != X64
		{
			Err(Err::new ("Binary is not an x64 binary"))
		}
		else if self.abi != SYSTEM_V_ABI
		{
			Err(Err::new ("Binary does not use system V abi"))
		}
		else if self.info != EXECUTABLE
		{
			Err(Err::new ("Binary is not an executable"))
		}
		else if self.phdr_entry_size as usize != size_of::<ProgramHeader> ()
		{
			Err(Err::new ("Invalid ELF program header sizes"))
		}
		else
		{
			Ok(())
		}
	}
}

const P_TYPE_NULL: u32 = 0;
const P_TYPE_LOAD: u32 = 1;
const P_TYPE_DYNAMIC: u32 = 2;
const P_TYPE_INTERP: u32 = 3;
const P_TYPE_NOTE: u32 = 4;

bitflags!
{
	pub struct PHdrFlags: u32
	{
		const EXECUTABLE = 1;
		const WRITABLE = 2;
		const READABLE = 4;
	}
}

impl PHdrFlags
{
	pub fn readable (&self) -> bool
	{
		self.contains (Self::READABLE)
	}

	pub fn writable (&self) -> bool
	{
		self.contains (Self::WRITABLE)
	}

	pub fn executable (&self) -> bool
	{
		self.contains (Self::EXECUTABLE)
	}
}

#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
struct ProgramHeader
{
	ptype: u32,
	flags: PHdrFlags,
	p_offset: usize,
	p_vaddr: usize,
	unused: usize,
	p_filesz: usize,
	p_memsz: usize,
	align: usize,
}
