pub enum Collections {
    Courses,
    CourseData,
    Courses2,
    Course2Data,
    Accounts,
    Votes,
    Meta,
}

impl Collections {
    pub fn as_str(&self) -> &str {
        match *self {
            Collections::Courses => "courses",
            Collections::CourseData => "courseData",
            Collections::Courses2 => "courses2",
            Collections::Course2Data => "course2Data",
            Collections::Accounts => "accounts",
            Collections::Votes => "votes",
            Collections::Meta => "meta",
        }
    }
}
