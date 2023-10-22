update campaign_coupon_types
set last_drawn_date = case
    when (last_drawn_date is not null and last_drawn_date != CURRENT_DATE) then CURRENT_DATE
    else last_drawn_date
end,
current_daily_quota = case
    when (last_drawn_date is not null and last_drawn_date != CURRENT_DATE) then daily_quota - 1
    else current_daily_quota - 1
end,
current_quota = current_quota - 1
where id = 3
returning *;

update campaign_coupon_types 
set last_drawn_date = CURRENT_DATE - interval '1 day'
where id = 3
returning *;

select *
from campaign_coupon_types;

select (last_drawn_date is not null and last_drawn_date = CURRENT_DATE)
from campaign_coupon_types
where id = 3;

select *
from campaign_coupons;

select *
from users;

delete from users
where phone = '+852 0000 0000';