pub trait AsBytes: Sized {
    const MEM_SIZE: usize = std::mem::size_of::<Self>();
    fn as_bytes(&self) -> &[u8; Self::MEM_SIZE] {
        let self_as_bytes = self as *const Self as *const [u8; Self::MEM_SIZE];
        unsafe { &*self_as_bytes }
    }
    fn as_bytes_mut(&mut self) -> &mut [u8; Self::MEM_SIZE] {
        let self_as_bytes = self as *mut Self as *mut [u8; Self::MEM_SIZE];
        // Safety: Handshake is a POD with repr(c)
        unsafe { &mut *self_as_bytes }
    }
}
