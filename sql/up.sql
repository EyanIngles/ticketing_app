CREATE TABLE comments(
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                ticket_id INTEGER NOT NULL,
                text TEXT NOT NULL,

                FOREIGN KEY (ticket_id) REFERENCES tickets(id)
);

