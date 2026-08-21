-- Add migration script here
create table if not exists waitlist (
    email text not null primary key,
    created_at timestamp not null default current_timestamp
);