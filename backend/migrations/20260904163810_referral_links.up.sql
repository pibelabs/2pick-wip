-- Add up migration script here
create table referral_links (
    id uuid not null unique,
    creator int references waitlist (id) not null,
    created_at timestamptz not null default now()
);

alter table waitlist add column referred_by int references waitlist (id);