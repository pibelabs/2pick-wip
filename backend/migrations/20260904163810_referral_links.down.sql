-- Add down migration script here
alter table waitlist drop column referred_by;

drop table referral_links;