drop table waitlist;

create table waitlist (
    id         integer primary key,
    email      text not null unique,
    created_at timestamp not null default current_timestamp
);

create table deregistration_links (
    id      integer primary key,
    value   text not null unique,
    user_id integer not null unique references waitlist(id)
);