-- Login sessions, backing the `session` cookie.
--
-- The cookie holds a random token; we only ever store its SHA-256 hash, so a leaked database dump can't be used
-- to impersonate anyone. Logging out is deleting a row, and "log out everywhere" is
--
-- delete from session where user_id = $1
create table session (
    session_id uuid primary key default uuidv7(),

    -- Deleting a user also logs them out everywhere.
    user_id uuid not null references "user" (user_id) on delete cascade,

    token_hash bytea unique not null,

    -- Sliding expiry: pushed forward (at most once a day) whenever the session is used.
    -- Expired rows are ignored by lookups and cleaned up lazily on the user's next login.
    expires_at timestamptz not null,

    created_at timestamptz not null default now()
);

create index on session (user_id);
