#[derive(Debug)]
pub struct UCircularIndex {
    current: usize,
    max: usize,
}

// TODO: Error handling
impl UCircularIndex {
    pub fn new(current: usize, max: usize) -> Self {
        Self { current, max }
    }
}

impl<N> std::ops::AddAssign<N> for UCircularIndex
where
    N: Into<i32>,
{
    fn add_assign(&mut self, rhs: N) {
        let mut curr = self.current as i32;

        curr += rhs.into();

        if curr < 0 {
            curr += self.max as i32;
        }

        self.current = curr as usize % self.max;
    }
}

impl AsRef<usize> for UCircularIndex {
    fn as_ref(&self) -> &usize {
        &self.current
    }
}
