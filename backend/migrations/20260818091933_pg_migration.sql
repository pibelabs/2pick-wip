-- Add migration script here
create table waitlist (
    id int generated always as identity primary key,
    email text not null unique,
    created_at timestamptz not null default now()
);

create table deregistration_links (
    id uuid primary key,
    user_id int references waitlist (id) on delete cascade not null unique
);