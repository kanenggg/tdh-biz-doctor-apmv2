FROM postgres:14-alpine AS pgdata-init

COPY ./dbscript /dbscript
RUN chmod +x /dbscript/ -R

ENV PATH=$PATH:/dbscript
ENV PGDATA=/pgdata

COPY ./entrypoint-sandbox.sh /docker-entrypoint-initdb.d/001-entrypoint-sandbox.sh
COPY ./sql/ /docker-entrypoint-initdb.d/sql/ 

RUN echo "exit 0" > /docker-entrypoint-initdb.d/9999999-exit.sh
RUN POSTGRES_PASSWORD=password docker-entrypoint.sh postgres && echo "done"

FROM postgres:14-alpine

ENV PGDATA=/pgdata

COPY --chown=postgres:postgres --from=pgdata-init /pgdata /pgdata

