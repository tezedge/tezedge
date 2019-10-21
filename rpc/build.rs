use git2::Repository;

fn main() {
    if let Ok( repo_path) = std::env::var("CARGO_MANIFEST_DIR") {
        if let Ok(repo) = Repository::open(repo_path) {
            if let Ok(head) = repo.head() {
                // head
            }
        }
    }
}