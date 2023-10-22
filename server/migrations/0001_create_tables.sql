CREATE TABLE users (
    id SERIAL PRIMARY KEY,
    phone VARCHAR(20) NOT NULL UNIQUE
);

CREATE TABLE campaigns (
    id SERIAL PRIMARY KEY
);

CREATE TABLE campaign_coupon_types (
    id SERIAL PRIMARY KEY,
    campaign_id INT NOT NULL,
    description TEXT NOT NULL,
    probability FLOAT4 NOT NULL,
    total_quota INT,
    daily_quota INT,
    current_quota INT,
    current_daily_quota INT,
    last_drawn_date DATE,

    FOREIGN KEY (campaign_id) REFERENCES campaigns (id) ON DELETE RESTRICT,
    CHECK (probability <= 1),
    CHECK (probability >= 0),
    CHECK (total_quota >= 0),
    CHECK (daily_quota >= 0),
    CHECK (current_quota >= 0),
    CHECK (current_daily_quota >= 0)
);

CREATE TABLE campaign_coupons (
    id SERIAL PRIMARY KEY,
    campaign_coupon_type_id INT NOT NULL,
    redeem_code TEXT NOT NULL UNIQUE,
    redeemed BOOLEAN NOT NULL DEFAULT FALSE,

    FOREIGN KEY (campaign_coupon_type_id) REFERENCES campaign_coupon_types (id) ON DELETE RESTRICT
);

CREATE TABLE draws (
    id SERIAL PRIMARY KEY,
    user_id INT NOT NULL,
    campaign_id INT NOT NULL,
    campaign_coupon_id INT,
    date DATE NOT NULL DEFAULT CURRENT_DATE,

    FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE SET NULL,
    FOREIGN KEY (campaign_id) REFERENCES campaigns (id) ON DELETE RESTRICT,
    FOREIGN KEY (campaign_coupon_id) REFERENCES campaign_coupons (id) ON DELETE RESTRICT
);