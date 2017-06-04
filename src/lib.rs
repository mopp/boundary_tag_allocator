#![no_std]
mod memory_region;

use memory_region::MemoryRegion;
use core::{mem, ptr};


trait allocator {
    fn malloc(usize) -> usize;
    fn free(usize);
}


struct MemoryManager<'a> {
    memory_regions: &'a [MemoryRegion],
    head: &'a BoundaryTag,
}


impl<'a> MemoryManager<'a> {
    fn new(mems: &'a [MemoryRegion]) -> MemoryManager
    {
        assert!(mems.len() != 0);

        // TODO: use all memory regions.
        let addr = mems[0].addr();
        let size = mems[0].size();

        MemoryManager {
            memory_regions: mems,
            head: BoundaryTag::from_memory(addr, size),
        }
    }
}


#[repr(C)]
#[derive(Debug)]
struct BoundaryTag {
    is_alloc: bool,
    is_sentinel: bool,
    size: usize,
}


impl<'a> BoundaryTag {
    fn from_memory(addr: usize, size: usize) -> &'a mut BoundaryTag
    {
        let tag = unsafe { &mut *(addr as *mut BoundaryTag) };

        tag.is_alloc = false;
        tag.is_sentinel = true;
        tag.size = size;

        tag
    }
}


#[cfg(test)]
mod tests {
    use core::{mem, ptr};

    #[test]
    fn test_all()
    {
        let x:[usize; 4096] = unsafe { mem::zeroed() };
    }

    fn test_memory_manager_fail()
    {

    }
}
