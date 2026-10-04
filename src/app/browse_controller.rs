use super::{AuthorState, HomeSidebarState, HomeState, PlaylistState, PrivateRoamState};

pub(crate) struct BrowseController {
    pub home: HomeState,
    pub home_sidebar: HomeSidebarState,
    pub playlist: PlaylistState,
    pub private_roam: PrivateRoamState,
    pub author: AuthorState,
}

impl Default for BrowseController {
    fn default() -> Self {
        Self {
            home: HomeState::default(),
            home_sidebar: HomeSidebarState::default(),
            playlist: PlaylistState::default(),
            private_roam: PrivateRoamState::default(),
            author: AuthorState::default(),
        }
    }
}

impl BrowseController {
    pub fn reset_pages(&mut self) {
        self.playlist = PlaylistState::default();
        self.author = AuthorState::default();
        self.home = HomeState::default();
        self.home_sidebar = HomeSidebarState::default();
        self.private_roam = PrivateRoamState::default();
    }
}
