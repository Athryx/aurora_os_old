use core::option::NoneError;
use sys_consts::SysErr;

pub trait Error
{
	fn get_error (&self) -> &str;
}

// error storing static str
#[derive(Debug)]
pub struct Err
{
	msg: &'static str,
}

impl Err
{
	pub fn new (msg: &'static str) -> Self
	{
		Err {msg}
	}
}

impl Error for Err
{
	fn get_error (&self) -> &str
	{
		self.msg
	}
}

impl From<NoneError> for Err
{
	fn from (_: NoneError) -> Self
	{
		Self::new ("none error")
	}
}

impl Error for SysErr
{
	fn get_error (&self) -> &str
	{
		self.as_str ()
	}
}
