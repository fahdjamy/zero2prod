-- Add migration script here
INSERT INTO users (user_id, username, password_hash)
VALUES ('ddf8994f-d522-4659-8d02-c1d479057be6',
        'admin',
        '$argon2id$v=19$m=15000,t=2,p=1$rOcm5uQvpzdvD6bBvJy7Gg$YOo8zc9oc123LGiMCU7GbI2Rsk7eBchtcsxPv+fSO/I');
