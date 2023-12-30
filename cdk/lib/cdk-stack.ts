import * as cdk from 'aws-cdk-lib';
import { Construct } from 'constructs';
import * as s3 from 'aws-cdk-lib/aws-s3';
import * as ec2 from 'aws-cdk-lib/aws-ec2';
import * as iam from 'aws-cdk-lib/aws-iam';

export class CombustionStack extends cdk.Stack {
  constructor(scope: Construct, id: string, props?: cdk.StackProps) {
    super(scope, id, props);

    // S3 Bucket
    const name = this.node.tryGetContext('name');
    if (!name) {
      throw new Error('"name" is required');
    }
    const bucket = new s3.Bucket(this, 'CombustionBucket', {
      bucketName: `${name}-combustion`,
      versioned: true,
    });

    // IAM Role for EC2
    const ec2Role = new iam.Role(this, 'InstanceRole', {
      assumedBy: new iam.ServicePrincipal('ec2.amazonaws.com'),
    });

    // Allow EC2 to read from S3
    bucket.grantRead(ec2Role);

    // EC2 Instance
    const vpc = new ec2.Vpc(this, 'Vpc', {
      maxAzs: 2, // Adjust based on your requirements
    });

    const instance = new ec2.Instance(this, 'Instance', {
      vpc,
      instanceType: ec2.InstanceType.of(ec2.InstanceClass.T2, ec2.InstanceSize.MICRO),
      machineImage: ec2.MachineImage.latestAmazonLinux(),
      role: ec2Role,
    });

    // IAM User
    const user = new iam.User(this, 'User');

    // Output the S3 bucket name
    new cdk.CfnOutput(this, 'Output', {
      value: bucket.bucketName,
    });
  }
}
