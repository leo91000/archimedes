pub trait StringUtils {
    fn split_3<'a>(&'a self, p: &str) -> Option<(&'a str, &'a str, &'a str)>;
    fn split_4<'a>(&'a self, p: &str) -> Option<(&'a str, &'a str, &'a str, &'a str)>;
}

impl StringUtils for String {
    fn split_3<'a>(&'a self, p: &str) -> Option<(&'a str, &'a str, &'a str)> {
        let (p1, rest) = self.split_once(p)?;
        let (p2, p3) = rest.split_once(p)?;
        Some((p1, p2, p3))
    }

    fn split_4<'a>(&'a self, p: &str) -> Option<(&'a str, &'a str, &'a str, &'a str)> {
        let (p1, p2, rest) = self.split_3(p)?;
        let (p3, p4) = rest.split_once(p)?;
        Some((p1, p2, p3, p4))
    }
}

impl StringUtils for &str {
    fn split_3<'a>(&'a self, p: &str) -> Option<(&'a str, &'a str, &'a str)> {
        let (p1, rest) = self.split_once(p)?;
        let (p2, p3) = rest.split_once(p)?;
        Some((p1, p2, p3))
    }

    fn split_4<'a>(&'a self, p: &str) -> Option<(&'a str, &'a str, &'a str, &'a str)> {
        let (p1, p2, rest) = self.split_3(p)?;
        let (p3, p4) = rest.split_once(p)?;
        Some((p1, p2, p3, p4))
    }
}
