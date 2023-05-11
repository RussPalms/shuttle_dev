use aws_sdk_iam::operation::create_policy::CreatePolicyError;
use aws_sdk_rds::{
    error::SdkError,
    operation::{
        create_db_instance::CreateDBInstanceError, describe_db_instances::DescribeDBInstancesError,
    },
};
use thiserror::Error;
use tonic::Status;
use tracing::error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("failed to create role: {0}")]
    CreateRole(String),

    #[error("failed to update role: {0}")]
    UpdateRole(String),

    #[error("failed to drop role: {0}")]
    DeleteRole(String),

    #[error("failed to create DB: {0}")]
    CreateDB(String),

    #[error("failed to drop DB: {0}")]
    DeleteDB(String),

    #[error("unexpected sqlx error: {0}")]
    UnexpectedSqlx(#[from] sqlx::Error),

    #[error("unexpected mongodb error: {0}")]
    UnexpectedMongodb(#[from] mongodb::error::Error),

    #[error("failed to create RDS instance: {0}")]
    CreateRDSInstance(#[from] SdkError<CreateDBInstanceError>),

    #[error("failed to get description of RDS instance: {0}")]
    DescribeRDSInstance(#[from] SdkError<DescribeDBInstancesError>),

    #[error("failed to create IAM policy for AWS: {0}")]
    CreateIAMPolicy(#[from] CreatePolicyError),

    #[error["plain error: {0}"]]
    Plain(String),
}

unsafe impl Send for Error {}

impl From<Error> for Status {
    fn from(err: Error) -> Self {
        error!(error = &err as &dyn std::error::Error, "provision failed");
        Status::internal("failed to provision a database")
    }
}
