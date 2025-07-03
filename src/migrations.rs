use loki_migration::{Revision, revision};

pub const DATABASE_REVISIONS: &[Revision] = &[
    revision!(v00_initial),
    revision!(
        v01_create_news_table,
        r#"CREATE TABLE news (
            id SERIAL PRIMARY KEY UNIQUE,
            url TEXT,
            title TEXT,
            notes TEXT
        );"#,
        r#"DROP TABLE IF EXISTS news"#
    ),
    revision!(
        v02_seed_news_table,
        r#"INSERT INTO news
            (url, title, notes)
        VALUES
            (
                'https://idahocapitalsun.com/2025/03/11/idaho-senate-signs-off-on-proposed-anti-marijuana-constitutional-amendment/',
                'Idaho Senate signs off on proposed anti-marijuana constitutional amendment',
                NULL
            );"#
    ),
    revision!(
        v03_add_hidden_to_news,
        "ALTER TABLE news ADD hidden BOOLEAN NOT NULL DEFAULT FALSE;",
        "ALTER TABLE news DROP COLUMN hidden;"
    ),
    revision!(
        v04_seed_news_table,
        r#"INSERT INTO news
            (url, title, notes)
        VALUES
            (
                'https://www.idahostatesman.com/news/business/article309074875.html',
                'https://www.idahostatesman.com/news/business/article309074875.html',
                NULL
            ),
            (
                'https://stateline.org/2025/04/15/marijuana-legalization-hits-roadblocks-after-years-of-expansion/',
                'https://stateline.org/2025/04/15/marijuana-legalization-hits-roadblocks-after-years-of-expansion/',
                NULL
            );
        "#
    ),
];
