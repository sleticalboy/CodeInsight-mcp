pub struct RenderService;

impl RenderService {
    pub fn render(&self) -> &'static str {
        "ok"
    }
}

pub fn build_service() -> RenderService {
    RenderService
}
