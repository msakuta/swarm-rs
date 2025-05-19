pub(crate) trait Idx {
    #[allow(dead_code)]
    fn wrapping_idx(&self, x: isize, y: isize) -> usize;

    #[allow(dead_code)]
    fn try_idx(&self, x: isize, y: isize) -> Option<usize>;

    /// Panics on out of bounds
    fn idx(&self, x: impl Into<isize>, y: impl Into<isize>) -> usize;
}

pub(crate) type Shape = (isize, isize);

impl Idx for Shape {
    fn wrapping_idx(&self, x: isize, y: isize) -> usize {
        let (width, height) = self;
        ((x + width) % width + (y + height) % height * width) as usize
    }

    fn try_idx(&self, x: isize, y: isize) -> Option<usize> {
        let (width, height) = self;
        if x < 0 || *width <= x || y < 0 || *height <= y {
            None
        } else {
            Some((x + y * width) as usize)
        }
    }

    fn idx(&self, x: impl Into<isize>, y: impl Into<isize>) -> usize {
        let (x, y) = (x.into(), y.into());
        let (width, height) = self;
        assert!(
            !(x < 0 || *width as isize <= x || y < 0 || *height as isize <= y),
            "Index out of bounds"
        );
        (x + y * width) as usize
    }
}

pub(crate) type Size = (usize, usize);

impl Idx for Size {
    fn wrapping_idx(&self, x: isize, y: isize) -> usize {
        let (width, height) = self;
        (x as usize + width) % width + (y as usize + height) % height * width
    }

    fn try_idx(&self, x: isize, y: isize) -> Option<usize> {
        let (width, height) = self;
        if x < 0 || *width as isize <= x || y < 0 || *height as isize <= y {
            None
        } else {
            Some(x as usize + y as usize * width)
        }
    }

    fn idx(&self, x: impl Into<isize>, y: impl Into<isize>) -> usize {
        let (x, y) = (x.into(), y.into());
        let (width, height) = self;
        assert!(
            !(x < 0 || *width as isize <= x || y < 0 || *height as isize <= y),
            "Index out of bounds"
        );
        x as usize + y as usize * width
    }
}
