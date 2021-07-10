use crate::uses::*;
use alloc::collections::BTreeMap;

// TODO: support all options later
#[derive(Debug, Clone, Copy)]
pub struct DomainHandler
{
	// domain handler function
	rip: usize,
	options: HandlerOptions,
}

impl DomainHandler
{
	pub const fn new (rip: usize, options: HandlerOptions) -> Self
	{
		DomainHandler {
			rip,
			options,
		}
	}

	pub fn rip (&self) -> usize
	{
		self.rip
	}

	pub fn options (&self) -> &HandlerOptions
	{
		&self.options
	}
}

#[derive(Debug, Clone, Copy)]
pub enum BlockMode
{
	// a new thread will be spawned to handler the incoming message
	NonBlocking,
	// the thread with tid usize will block and handler the incoming message
	Blocking(usize),
}

#[derive(Debug, Clone, Copy)]
pub struct HandlerOptions
{
	// If this is true, will block the 
	pub blocking_mode: BlockMode,
}

impl HandlerOptions
{
	pub fn new () -> Self
	{
		HandlerOptions {
			blocking_mode: BlockMode::NonBlocking,
		}
	}
}

#[derive(Debug)]
pub struct DomainMap
{
	// function to call if domain is called and no handler is registered for it
	default_handler: Option<DomainHandler>,
	domains: BTreeMap<usize, DomainHandler>,
}

impl DomainMap
{
	pub const fn new () -> Self
	{
		DomainMap {
			default_handler: None,
			domains: BTreeMap::new (),
		}
	}

	pub fn default_handler (&self) -> Option<DomainHandler>
	{
		self.default_handler
	}

	// returns old default handler
	pub fn set_default_handler (&mut self, new: Option<DomainHandler>) -> Option<DomainHandler>
	{
		let out = self.default_handler;
		self.default_handler = new;
		out
	}

	// if domain is none, will set the default handler
	// returns the old handler if one was registered
	pub fn register (&mut self, domain: Option<usize>, handler: DomainHandler) -> Option<DomainHandler>
	{
		match domain
		{
			Some(domain) =>	self.domains.insert (domain, handler),
			None => self.set_default_handler (Some(handler)),
		}
	}

	// removes specified domain handler, returning the old one if it existed
	pub fn remove (&mut self, domain: Option<usize>) -> Option<DomainHandler>
	{
		match domain
		{
			Some(domain) => self.domains.remove (&domain),
			None => self.set_default_handler (None),
		}
	}

	pub fn get (&self, domain: usize) -> Option<&DomainHandler>
	{
		self.domains.get (&domain).or ((&self.default_handler).as_ref ())
	}
}
