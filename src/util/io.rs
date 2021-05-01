use core::fmt::{self, Write};
use core::cell::UnsafeCell;
use volatile::Volatile;
use lazy_static::lazy_static;
use spin::Mutex;
use crate::arch::x64::*;
use crate::consts;

const VGA_BUF_WIDTH: usize = 80;
const VGA_BUF_HEIGHT: usize = 25;

const DEBUGCON_PORT: u16 = 0xe9;

lazy_static!
{
	pub static ref WRITER: Mutex<Writer> = Mutex::new (Writer
	{
		xpos: 0,
		ypos: 0,
		color: ColorCode::new (Color::Yellow, Color::Black),
		buffer: unsafe { ((*consts::KERNEL_VMA + 0xb8000) as *mut Buffer).as_mut ().unwrap () },
	});
}
pub static E_WRITER: Mutex<PortWriter> = Mutex::new (PortWriter::new (DEBUGCON_PORT));
// doesn't lock, so ideal for calling from interrupt handlers, but it is not synchronized
pub static mut R_WRITER: PortWriter = PortWriter::new (DEBUGCON_PORT);


#[repr(transparent)]
struct Buffer
{
	chars: [[Volatile<ScreenChar>; VGA_BUF_WIDTH]; VGA_BUF_HEIGHT],
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Color
{
	Black = 0,
	Blue = 1,
	Green = 2,
	Cyan = 3,
	Red = 4,
	Magenta = 5,
	Brown = 6,
	LightGray = 7,
	DarkGray = 8,
	LightBlue = 9,
	LightGreen = 10,
	LightCyan = 11,
	LightRed = 12,
	Pink = 13,
	Yellow = 14,
	White = 15,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
struct ColorCode(u8);

impl ColorCode
{
	const fn new (foreground: Color, background: Color) -> Self
	{
		ColorCode((background as u8) << 4 | (foreground as u8))
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
struct ScreenChar
{
	cchar: u8,
	color: ColorCode,
}

impl ScreenChar
{
	fn new (cchar: u8, color: ColorCode) -> Self
	{
		ScreenChar {cchar, color}
	}
}

pub struct Writer
{
	xpos: usize,
	ypos: usize,
	color: ColorCode,
	buffer: &'static mut Buffer,
}

impl Writer
{
	// when this is called previous calls would have gauranteed xpos and ypos are correct
	pub fn write_byte (&mut self, byte: u8)
	{
		match byte
		{
			b'\n' => {
				self.ypos += 1;
				self.xpos = 0;
				self.wrap_pos ();
			},
			_ => {
				let ctow = ScreenChar::new (byte, self.color);
				self.buffer.chars[self.ypos][self.xpos].write (ctow);
				self.xpos += 1;
				self.wrap_pos ();
			},
		}
	}

	pub fn write_string (&mut self, string: &str)
	{
		for b in string.bytes ()
		{
			match b
			{
				0x20..=0x7e | b'\n' => self.write_byte (b),
				_ => self.write_byte (0xfe),
			}
		}
	}

	pub fn clear (&mut self)
	{
		for y in 0..VGA_BUF_HEIGHT
		{
			self.clear_row (y);
		}
	}

	fn scroll_down (&mut self, lines: usize)
	{
		if lines >= VGA_BUF_HEIGHT
		{
			for y in 0..VGA_BUF_HEIGHT
			{
				self.clear_row (y);
			}
			return;
		}

		for y in 0..(VGA_BUF_HEIGHT - lines)
		{
			for x in 0..VGA_BUF_WIDTH
			{
				let buf = &mut self.buffer.chars;
				buf[y][x].write (buf[y + lines][x].read ());
			}
		}

		for y in (VGA_BUF_HEIGHT - lines)..VGA_BUF_HEIGHT
		{
			self.clear_row (y);
		}
	}

	fn clear_row (&mut self, row: usize)
	{
		let blank = ScreenChar::new (b' ', self.color);

		for x in 0..VGA_BUF_WIDTH
		{
			self.buffer.chars[row][x].write (blank);
		}
	}

	fn wrap_pos (&mut self)
	{
		if self.xpos >= VGA_BUF_WIDTH
		{
			self.xpos = 0;
			self.ypos += 1;
		}
		if self.ypos >= VGA_BUF_HEIGHT
		{
			self.scroll_down (self.ypos - VGA_BUF_HEIGHT + 1);
			self.ypos = VGA_BUF_HEIGHT - 1;
		}
	}
}

impl Write for Writer
{
	fn write_str (&mut self, s: &str) -> fmt::Result
	{
		self.write_string (s);
		Ok(())
	}
}

#[macro_export]
macro_rules! print {
	($($arg:tt)*) => ($crate::util::io::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
	() => ($crate::print!("\n"));
	($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

#[doc(hidden)]
pub fn _print (args: fmt::Arguments)
{
	WRITER.lock ().write_fmt (args).unwrap ();
}

pub struct PortWriter
{
	port: u16,
}

impl PortWriter
{
	const fn new (port: u16) -> Self
	{
		PortWriter {
			port
		}
	}

	pub fn write_byte (&self, byte: u8)
	{
		outb (self.port, byte);
	}

	pub fn write_string (&self, string: &str)
	{
		for b in string.bytes ()
		{
			self.write_byte (b);
		}
	}
}

impl Write for PortWriter
{
	fn write_str (&mut self, s: &str) -> fmt::Result
	{
		self.write_string (s);
		Ok(())
	}
}

#[macro_export]
macro_rules! eprint {
	($($arg:tt)*) => ($crate::util::io::_eprint(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! eprintln {
	() => ($crate::eprint!("\n"));
	($($arg:tt)*) => ($crate::eprint!("{}\n", format_args!($($arg)*)));
}

#[doc(hidden)]
pub fn _eprint (args: fmt::Arguments)
{
	E_WRITER.lock ().write_fmt (args).unwrap ();
}

#[macro_export]
macro_rules! rprint {
	($($arg:tt)*) => ($crate::util::io::_rprint(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! rprintln {
	() => ($crate::rprint!("\n"));
	($($arg:tt)*) => ($crate::rprint!("{}\n", format_args!($($arg)*)));
}

#[doc(hidden)]
pub fn _rprint (args: fmt::Arguments)
{
	unsafe
	{
		R_WRITER.write_fmt (args).unwrap ();
	}
}
