use loki_migration::{Revision, revision};

pub const DATABASE_REVISIONS: &[Revision] = &[
    revision!("000_initial"),
    revision!(
        "001_create_news_table",
        r#"
        CREATE TABLE news (
            id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
            url TEXT,
            title TEXT,
            notes TEXT
        )
        "#,
        "DROP TABLE IF EXISTS news"
    ),
    revision!(
        "002_seed_news_table",
        r#"INSERT INTO news
            (url, title, notes)
        VALUES
            (
                'https://idahocapitalsun.com/2025/03/11/idaho-senate-signs-off-on-proposed-anti-marijuana-constitutional-amendment/',
                'Idaho Senate signs off on proposed anti-marijuana constitutional amendment',
                NULL
            ),
            (
                'https://www.idahostatesman.com/news/business/article309074875.html',
                '‘Not giving up on legalization,’ this Boise CBD dispensary is closing',
                'The Honey Pot CBD in Boise is closing this June but will shift to online sales. The shop promises to continue advocating for cannabis legalization in Idaho.'
            ),
            (
                'https://stateline.org/2025/04/15/marijuana-legalization-hits-roadblocks-after-years-of-expansion/',
                'Marijuana legalization hits roadblocks after years of expansion',
                'Though most states have legalized some use of marijuana, lawmakers have increasingly targeted the drug this year.'
            )            
            "#
    ),
    revision!(
        "003_add_hidden_to_news",
        "ALTER TABLE news ADD hidden BOOLEAN NOT NULL DEFAULT FALSE;",
        "ALTER TABLE news DROP COLUMN hidden;"
    ),
    revision!(
        "004_add_users",
        r#"
        CREATE TABLE users (
            id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
            email TEXT UNIQUE NOT NULL,
            password BYTEA NOT NULL
        );
        
        CREATE INDEX idx_users_email ON users (email);

        CREATE TABLE refresh_tokens (
            id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
            user_id BIGINT NOT NULL,
            token BYTEA NOT NULL,
            CONSTRAINT fk_refresh_tokens_users FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
        )
        "#,
        r#"
        DROP TABLE users; 
        DROP_TABLE refresh_tokens
        "#
    ),
];
