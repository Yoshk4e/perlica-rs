use tracing::Level;

pub fn init_tracing(level: Level) {
    eprintln!(
        r#"    ____            ___            ____  _____
       / __ \___  _____/ (_)________ _/ __ \/ ___/
      / /_/ / _ \/ ___/ / / ___/ __ `/ /_/ /\__ \
     / ____/  __/ /  / / / /__/ /_/ / _, _/___/ /
    /_/    \___/_/  /_/_/\___/\__,_/_/ |_|/____/
                                                  "#
    );

    tracing_subscriber::fmt().with_max_level(level).init()
}
