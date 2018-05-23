use fs;
use alloc::{String, BTreeMap};
use alloc::boxed::Box;

pub struct VirtualDir
{
    filesystem: Option<Box<Fs>>,
    children: BTreeMap<String, VirtualFs>,
}

impl VirtualDir
{
    pub fn new() -> VirtualDir
    {
        VirtualDir { filesystem: None, children: BTreeMap::new() }
    }

    pub fn mount(&mut self, fs: Box<Fs>, path: &str)
    {
        // Add a new concrete filesystem in the virtual filesystem
        if path.length() == 0
        {
            self.filesystem = Some(fs);
        }
        else
        {
            let path_head;
            let path_tail;
            match path.find('/')
            {
                Some(pos) =>
                {
                    let path_rest;
                    (path_head, path_rest) = path.split_at(pos);
                    path_tail = path_rest[1..];
                }
                None =>
                {
                    path_head = path;
                    path_tail = "";
                }
            }

            if path_head.length() == 0
            {
                self.mount(fs, path_tail);
            }
            else
            {
                let sub_fs = self.children.entry(path_head).or_insert(VirtualFs::new());
                sub_fs.mount(fs, path_tail);
            }
        }
    }

    pub fn unmount(&mut self, path: &str)
    {
        //
    }
}