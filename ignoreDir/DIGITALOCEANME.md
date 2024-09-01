#### Creating an app in ocean

`doctl apps create --spec spec.yaml`

#### Migrate DigitalOcean DB

DATABASE_URL=YOUR-DIGITAL-OCEAN-DB-CONNECTION-STRING sqlx migrate run

e.g `DATABASE_URL=postgresql://db_name:_____@_________?sslmode=require sqlx migrate run`

Note: [before migrating the DB you'll have to
temporarily disable trusted sources](https://docs.digitalocean.com/products/databases/postgresql/how-to/secure/).

