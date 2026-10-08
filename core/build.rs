fn main() {
    println!("cargo:rerun-if-env-changed=GITHUBSP_RELEASE_REPOSITORY");
}
