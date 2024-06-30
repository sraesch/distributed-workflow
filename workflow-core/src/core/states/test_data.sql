-- Insert some jobs
SELECT
    create_job (
        'f47e3b2d-4e9b-4b3d-9f3e-3d0c4f1f5f3a',
        'search_job',
        '{}',
        '2020-03-12 13:12:41 +01:00'
    );

SELECT
    create_job (
        '03bec38a-67e2-4223-a14c-f6503e6aa1cd',
        'search_job',
        '{}',
        '2020-03-12 13:13:15 +01:00'
    );

SELECT
    create_job (
        'a09a26d8-b826-4bc6-91a6-12d249afa861',
        'compress_job',
        '{}',
        '2020-03-13 14:02:00 +01:00'
    );

SELECT
    create_job (
        '4ee8f33c-745b-4310-b98c-a440555e87f0',
        'compress_job',
        '{}',
        '2020-03-13 14:10:00 +01:00'
    );

-- Change state of the jobs to running
SELECT
    change_job_state(
        'f47e3b2d-4e9b-4b3d-9f3e-3d0c4f1f5f3a',
        2,
        0,
        '2020-03-12 13:12:42 +01:00'
    );

SELECT
    change_job_state(
        '03bec38a-67e2-4223-a14c-f6503e6aa1cd',
        2,
        0,
        '2020-03-12 13:13:16 +01:00'
    );

SELECT
    change_job_state(
        'a09a26d8-b826-4bc6-91a6-12d249afa861',
        2,
        0,
        '2020-03-13 14:02:01 +01:00'
    );

SELECT
    change_job_state(
        '4ee8f33c-745b-4310-b98c-a440555e87f0',
        2,
        0,
        '2020-03-13 14:10:01 +01:00'
    );

-- Insert tasks
SELECT
    create_task(
        'f47e3b2d-4e9b-4b3d-9f3e-3d0c4f1f5f3a',
        'c62fb9dc-2c01-4106-8676-ba6dace41e59',
        'search',
        '{"key": "foobar"}',
        '2020-03-12 13:13:00 +01:00'
    );

SELECT
    create_task(
        'f47e3b2d-4e9b-4b3d-9f3e-3d0c4f1f5f3a',
        'cfa196e3-ffd4-463e-9a61-a8258787ea2f',
        'search',
        '{"key": "foobar2"}',
        '2020-03-12 13:13:00 +01:00'
    );

SELECT
    create_task(
        '03bec38a-67e2-4223-a14c-f6503e6aa1cd',
        '5de93e18-fb39-420b-8324-9291cdbf4d97',
        'search',
        '{"key": "42"}',
        '2020-03-12 13:14:00 +01:00'
    );

SELECT
    create_task(
        'a09a26d8-b826-4bc6-91a6-12d249afa861',
        '6dde865c-3584-4f7c-a387-15392a013045',
        'compress',
        '{"input_file": "file1.txt", "compression_level": "9"}',
        '2020-03-12 13:14:00 +01:00'
    );

SELECT
    create_task(
        '4ee8f33c-745b-4310-b98c-a440555e87f0',
        '8da89d77-e6c3-4b72-9d90-80d16bcb9660',
        'compress',
        '{"input_file": "file2.txt", "compression_level": "8"}',
        '2020-03-13 14:12:00 +01:00'
    );

SELECT
    create_task(
        '4ee8f33c-745b-4310-b98c-a440555e87f0',
        '4ef91bd8-6577-44a8-80ff-57a7647129b0',
        'compress',
        '{"input_file": "file3.txt", "compression_level": "8"}',
        '2020-03-13 14:12:00 +01:00'
    );

SELECT
    create_task(
        '4ee8f33c-745b-4310-b98c-a440555e87f0',
        '26c41ca4-d954-4bf6-88e4-83f78add0847',
        'compress',
        '{"input_file": "file4.txt", "compression_level": "8"}',
        '2020-03-13 14:12:00 +01:00'
    );

SELECT
    create_task(
        '4ee8f33c-745b-4310-b98c-a440555e87f0',
        'bac73ef1-1c82-46be-a949-e65fb2d74e77',
        'compress',
        '{"input_file": "file5.txt", "compression_level": "8"}',
        '2020-03-13 14:12:00 +01:00'
    );